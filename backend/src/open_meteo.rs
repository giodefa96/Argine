//! Open-Meteo rain-forecast ingestion for the Seveso basin.
//!
//! The basin is approximated by the coordinates of the three Seveso stations
//! (`arpa::SEVESO_STATIONS`) for the MVP — see IDEAS.md §12 Q3. For each station we fetch
//! the hourly forecast precipitation and store it as a `weather_forecast` run keyed by
//! `run_ts` (the fetch time): every run is kept, so later models can be backtested against
//! what was known at the time. Free, no API key. All HTTP is mocked with `wiremock` in tests.

use crate::arpa;
use crate::domain;
use serde::Deserialize;
use sqlx::PgPool;
use time::format_description::well_known::Iso8601;
use time::{OffsetDateTime, PrimitiveDateTime};

/// Default API base; overridden in tests with a wiremock URL.
pub const DEFAULT_BASE_URL: &str = "https://api.open-meteo.com/v1";
/// Open-Meteo's automatic model selection; stored on every run for provenance.
pub const MODEL: &str = "best_match";
/// Forecast horizon. The basin's concentration time is hours (IDEAS.md §2), so 3 days
/// comfortably covers the useful window.
const FORECAST_DAYS: u8 = 3;

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("db error: {0}")]
    Db(#[from] sqlx::Error),
}

/// The slice of the Open-Meteo response we use. With `timezone=UTC` the `time` strings
/// are naive ISO 8601 in UTC; `precipitation` entries can be `null` for missing hours.
#[derive(Debug, Deserialize)]
struct ForecastResponse {
    hourly: Hourly,
}

#[derive(Debug, Deserialize)]
struct Hourly {
    time: Vec<String>,
    precipitation: Vec<Option<f64>>,
}

/// Normalize a response → `(ts, rain_mm)` points. Drops hours with a `null` value or an
/// unparseable timestamp; the two arrays are zipped, so a length mismatch just truncates.
fn normalize(resp: &ForecastResponse) -> Vec<(OffsetDateTime, f64)> {
    resp.hourly
        .time
        .iter()
        .zip(&resp.hourly.precipitation)
        .filter_map(|(t, p)| {
            let rain_mm = (*p)?;
            let naive = PrimitiveDateTime::parse(t, &Iso8601::DEFAULT).ok()?;
            Some((naive.assume_utc(), rain_mm))
        })
        .collect()
}

/// Thin HTTP client for the Open-Meteo forecast API.
pub struct OpenMeteoClient {
    http: reqwest::Client,
    base_url: String,
}

impl OpenMeteoClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Hourly forecast precipitation at a point, normalized to `(ts, rain_mm)`.
    pub async fn fetch_rain_forecast(
        &self,
        lat: f64,
        lon: f64,
    ) -> Result<Vec<(OffsetDateTime, f64)>, reqwest::Error> {
        let url = format!("{}/forecast", self.base_url);
        let resp = self
            .http
            .get(url)
            .query(&[
                ("latitude", lat.to_string().as_str()),
                ("longitude", lon.to_string().as_str()),
                ("hourly", "precipitation"),
                ("forecast_days", &FORECAST_DAYS.to_string()),
                ("timezone", "UTC"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<ForecastResponse>()
            .await?;
        Ok(normalize(&resp))
    }
}

/// One forecast-poll cycle: seed the Seveso stations (idempotent, shared with ARPA), then
/// fetch and store one forecast run per station under `run_ts`. The caller supplies
/// `run_ts` (the fetch time) so polls are deterministic in tests. Returns points stored.
pub async fn poll_once(
    pool: &PgPool,
    client: &OpenMeteoClient,
    run_ts: OffsetDateTime,
) -> Result<usize, IngestError> {
    let map = arpa::seed_stations(pool).await?;
    let mut total = 0;
    for s in arpa::SEVESO_STATIONS {
        let points = client.fetch_rain_forecast(s.lat, s.lon).await?;
        total += domain::upsert_weather_forecasts(pool, map[s.sensor_id], MODEL, run_ts, &points)
            .await?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn resp(time: Vec<&str>, precipitation: Vec<Option<f64>>) -> ForecastResponse {
        ForecastResponse {
            hourly: Hourly {
                time: time.into_iter().map(String::from).collect(),
                precipitation,
            },
        }
    }

    #[test]
    fn normalize_parses_naive_times_as_utc() {
        let points = normalize(&resp(
            vec!["2026-06-04T00:00", "2026-06-04T01:00"],
            vec![Some(0.0), Some(2.5)],
        ));
        assert_eq!(
            points,
            vec![
                (datetime!(2026-06-04 00:00 UTC), 0.0),
                (datetime!(2026-06-04 01:00 UTC), 2.5),
            ]
        );
    }

    #[test]
    fn normalize_drops_null_values_and_bad_timestamps() {
        let points = normalize(&resp(
            vec!["2026-06-04T00:00", "not-a-time", "2026-06-04T02:00"],
            vec![None, Some(1.0), Some(3.0)],
        ));
        assert_eq!(points, vec![(datetime!(2026-06-04 02:00 UTC), 3.0)]);
    }

    #[test]
    fn normalize_truncates_on_length_mismatch() {
        // 3 timestamps, 2 values: zip stops at the shorter side instead of panicking.
        let points = normalize(&resp(
            vec!["2026-06-04T00:00", "2026-06-04T01:00", "2026-06-04T02:00"],
            vec![Some(1.0), Some(2.0)],
        ));
        assert_eq!(points.len(), 2);
    }
}
