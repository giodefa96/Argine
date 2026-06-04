//! Baseline level forecast (Phase 1 — no ML, IDEAS.md §6).
//!
//! The empirical form from the design doc: `level(t+h) ≈ level(t) + α·cum_rain(t..t+h) − β·h`
//! — current level, plus a rain response on the cumulative forecast rain, minus a linear
//! drainage term. Computed on the fly from the latest level observation and the latest
//! Open-Meteo run (nothing stored). Deliberately naive but interpretable; the key invariant
//! (more rain ⇒ never-lower predicted level, since α ≥ 0) is property-tested. ML replaces
//! the math in Phase 2 behind the same interface.

use crate::domain::{self, Metric};
use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;

/// Model identifier returned by the API (and stored, once forecasts are persisted).
pub const MODEL_VERSION: &str = "baseline_v0";
/// Rain response: metres of level rise per mm of rain at the station point.
/// **Uncalibrated placeholder** — to be fitted on historical events (Phase 2 backtesting).
pub const ALPHA_M_PER_MM: f64 = 0.02;
/// Drainage: metres of level decay per hour without rain. **Uncalibrated placeholder.**
pub const BETA_M_PER_H: f64 = 0.01;

/// One predicted level point.
#[derive(Debug, Clone, Serialize)]
pub struct ForecastPoint {
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub value_m: f64,
}

/// The inputs the prediction was computed from — surfaced so clients can judge staleness
/// (the ARPA level lags ~18 h; see DATA_SOURCES.md).
#[derive(Debug, Clone, Serialize)]
pub struct BasedOn {
    #[serde(with = "time::serde::rfc3339")]
    pub level_ts: OffsetDateTime,
    pub level_m: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub weather_run_ts: OffsetDateTime,
}

/// A station's level forecast. `points` is empty (and `based_on` null) while the station
/// has no level observation or no weather run yet — nothing to predict from.
#[derive(Debug, Clone, Serialize)]
pub struct StationForecast {
    pub station_id: i64,
    pub model: &'static str,
    pub based_on: Option<BasedOn>,
    pub points: Vec<ForecastPoint>,
}

/// The pure baseline: predicted level for each forecast hour, given the starting level and
/// the forecast rain per hour. `v[i] = level0 + α·Σ(rain[..=i]) − β·(i+1)`.
pub fn baseline(level0: f64, rain_mm: &[f64], alpha: f64, beta: f64) -> Vec<f64> {
    let mut cum = 0.0;
    rain_mm
        .iter()
        .enumerate()
        .map(|(i, r)| {
            cum += r;
            level0 + alpha * cum - beta * (i + 1) as f64
        })
        .collect()
}

/// Assemble a station's forecast: anchor on the latest observed level, then apply the
/// baseline over the future hours (`ts > now`) of the latest weather run, capped at
/// `hours`. `now` is a parameter so tests are deterministic.
pub async fn for_station(
    pool: &PgPool,
    station_id: i64,
    hours: usize,
    now: OffsetDateTime,
) -> sqlx::Result<StationForecast> {
    let level = domain::latest_observation(pool, station_id, Metric::LevelM).await?;
    let run = domain::latest_weather_forecast(pool, station_id).await?;
    let future: Vec<_> = run.into_iter().filter(|f| f.ts > now).take(hours).collect();

    let (Some(level), Some(first)) = (level, future.first()) else {
        return Ok(StationForecast {
            station_id,
            model: MODEL_VERSION,
            based_on: None,
            points: vec![],
        });
    };

    let rain: Vec<f64> = future.iter().map(|f| f.rain_mm).collect();
    let values = baseline(level.value, &rain, ALPHA_M_PER_MM, BETA_M_PER_H);
    let based_on = BasedOn {
        level_ts: level.ts,
        level_m: level.value,
        weather_run_ts: first.run_ts,
    };
    let points = future
        .iter()
        .zip(values)
        .map(|(f, value_m)| ForecastPoint { ts: f.ts, value_m })
        .collect();
    Ok(StationForecast {
        station_id,
        model: MODEL_VERSION,
        based_on: Some(based_on),
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn baseline_matches_the_documented_form() {
        // level 1.0 m, 10 mm then 0 mm, α=0.02, β=0.01:
        // h1: 1.0 + 0.02·10 − 0.01·1 = 1.19; h2: 1.0 + 0.02·10 − 0.01·2 = 1.18.
        let v = baseline(1.0, &[10.0, 0.0], 0.02, 0.01);
        assert_eq!(v, vec![1.19, 1.18]);
    }

    #[test]
    fn no_rain_is_pure_drainage() {
        let v = baseline(2.0, &[0.0, 0.0, 0.0], 0.02, 0.01);
        assert!(
            v.windows(2).all(|w| w[1] < w[0]),
            "drainage only: strictly decreasing"
        );
        assert_eq!(v[2], 2.0 - 0.03);
    }

    proptest! {
        /// The alert-safety invariant: more rain must NEVER lower the predicted level.
        #[test]
        fn more_rain_never_lowers_the_forecast(
            level in -2.0_f64..10.0,
            base in proptest::collection::vec(0.0_f64..50.0, 1..48),
            extra in proptest::collection::vec(0.0_f64..50.0, 1..48),
        ) {
            let n = base.len().min(extra.len());
            let less = &base[..n];
            let more: Vec<f64> = less.iter().zip(&extra[..n]).map(|(a, b)| a + b).collect();
            let v_less = baseline(level, less, ALPHA_M_PER_MM, BETA_M_PER_H);
            let v_more = baseline(level, &more, ALPHA_M_PER_MM, BETA_M_PER_H);
            for (lo, hi) in v_less.iter().zip(&v_more) {
                prop_assert!(hi >= lo, "rain added ⇒ forecast must not drop: {hi} < {lo}");
            }
        }

        /// One prediction per forecast hour, whatever the inputs.
        #[test]
        fn output_length_matches_horizon(
            level in -2.0_f64..10.0,
            rain in proptest::collection::vec(0.0_f64..50.0, 0..48),
        ) {
            prop_assert_eq!(baseline(level, &rain, ALPHA_M_PER_MM, BETA_M_PER_H).len(), rain.len());
        }
    }
}
