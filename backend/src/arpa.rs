//! ARPA Lombardia ingestion (Seveso): river level + co-located rainfall.
//!
//! Source: Regione Lombardia open-data (Socrata). There is no public real-time API for
//! lowland river levels, so published values lag ~18h — see `DATA_SOURCES.md`. Two paths:
//! a scheduled forward poll (recent dataset) and a one-shot historical backfill (for the
//! frontend time-slider / ML). For each station we ingest its hydrometric level and, when a
//! co-located rain gauge exists, its rainfall — stored on the SAME station as `rain_mm`, so
//! level and rain correlate at one place. All HTTP is mocked with `wiremock` in tests.

use crate::domain::{self, Metric, NewStation, StationKind};
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use time::macros::{format_description, offset};
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

/// `station.source` value for every station ingested from ARPA.
pub const SOURCE: &str = "arpa_lombardia";
/// Default Socrata base; overridden in tests with a wiremock URL.
pub const DEFAULT_BASE_URL: &str = "https://www.dati.lombardia.it/resource";
/// Recent values (2025→now) and digitized history (2021→2025) datasets.
const DATASET_LATEST: &str = "647i-nhxk";
const DATASET_HISTORY: &str = "3e8b-w7ay";
/// ARPA timestamps are in solar time (UTC+1), without an explicit offset.
const ARPA_OFFSET: UtcOffset = offset!(+1);
/// ARPA encodes missing readings as a large negative sentinel (-999/-9999).
const SENTINEL_MAX: f64 = -900.0;
/// Socrata's max rows per response — the backfill pages in this size.
const PAGE: u32 = 1000;
/// Divisor to the canonical stored unit. Level is reported in cm → metres (÷100); rain in
/// mm → kept as-is (÷1). A divisor (not a ×0.01 factor) keeps values like 188→1.88 exact.
const LEVEL_DIVISOR: f64 = 100.0;
const RAIN_DIVISOR: f64 = 1.0;

/// A Seveso station to ingest. `sensor_id` is the ARPA hydrometric `idsensore`, stored as
/// `station.external_id`. `rain_sensor_id` is the co-located rain gauge, ingested onto the
/// same station as `rain_mm`. Coordinates are from the ARPA registry (`nf78-nj6b`).
pub struct SeedStation {
    pub sensor_id: &'static str,
    pub rain_sensor_id: Option<&'static str>,
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

/// The active hydrometric stations on the Seveso (see `DATA_SOURCES.md`): upstream → city,
/// each paired with the nearest rain gauge (8199/30525 are co-located; 4065 is the closest
/// gauge to Niguarda).
pub const SEVESO_STATIONS: &[SeedStation] = &[
    SeedStation {
        sensor_id: "8119",
        rain_sensor_id: Some("8199"),
        name: "Cantù Asnago",
        lat: 45.71853181,
        lon: 9.10037269,
    },
    SeedStation {
        sensor_id: "8121",
        rain_sensor_id: Some("30525"),
        name: "Paderno Dugnano Palazzolo",
        lat: 45.58280558,
        lon: 9.15897783,
    },
    SeedStation {
        sensor_id: "3118",
        rain_sensor_id: Some("4065"),
        name: "Milano Niguarda",
        lat: 45.52580195,
        lon: 9.19222420,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("db error: {0}")]
    Db(#[from] sqlx::Error),
}

/// A raw row from either Socrata dataset (they share these fields).
#[derive(Debug, Deserialize)]
struct ArpaRow {
    #[allow(dead_code)] // present in the API payload; we key by the requested sensor instead
    idsensore: String,
    data: String,
    valore: String,
    #[serde(default)]
    #[allow(dead_code)] // kept for future quality filtering on the validity flag
    stato: Option<String>,
}

/// A normalized observation: value in the canonical unit at an absolute timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObs {
    pub ts: OffsetDateTime,
    pub value: f64,
}

/// Normalize raw ARPA rows → observations, dividing the raw reading by `divisor` (100 for
/// level cm→m, 1 for rain mm). Drops rows with an unparseable timestamp/value or the sentinel.
fn normalize(rows: &[ArpaRow], divisor: f64) -> Vec<ParsedObs> {
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]");
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let Ok(value) = r.valore.trim().parse::<f64>() else {
            continue;
        };
        if value <= SENTINEL_MAX {
            continue;
        }
        let Ok(naive) = PrimitiveDateTime::parse(&r.data, &fmt) else {
            continue;
        };
        out.push(ParsedObs {
            ts: naive.assume_offset(ARPA_OFFSET),
            value: value / divisor,
        });
    }
    out
}

/// Thin HTTP client for the ARPA open-data (Socrata) API.
pub struct ArpaClient {
    http: reqwest::Client,
    base_url: String,
}

impl ArpaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    async fn get(
        &self,
        dataset: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<ArpaRow>, reqwest::Error> {
        let url = format!("{}/{}.json", self.base_url, dataset);
        self.http
            .get(url)
            .query(query)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<ArpaRow>>()
            .await
    }

    /// The most recent `limit` observations for one sensor (forward poll), scaled to the unit.
    pub async fn fetch_recent(
        &self,
        sensor_id: &str,
        limit: u32,
        divisor: f64,
    ) -> Result<Vec<ParsedObs>, reqwest::Error> {
        let limit = limit.to_string();
        let rows = self
            .get(
                DATASET_LATEST,
                &[
                    ("idsensore", sensor_id),
                    ("$order", "data DESC"),
                    ("$limit", &limit),
                ],
            )
            .await?;
        Ok(normalize(&rows, divisor))
    }

    /// One page of a dataset's series for a sensor, ascending by time. Returns
    /// `(raw_row_count, observations)`: the raw count drives backfill termination, since a
    /// page can be non-empty yet normalize to nothing (e.g. an outage of all-sentinel rows).
    async fn fetch_page(
        &self,
        dataset: &str,
        sensor_id: &str,
        offset: u32,
        divisor: f64,
    ) -> Result<(usize, Vec<ParsedObs>), reqwest::Error> {
        let (limit, offset) = (PAGE.to_string(), offset.to_string());
        let rows = self
            .get(
                dataset,
                &[
                    ("idsensore", sensor_id),
                    ("$order", "data ASC"),
                    ("$limit", &limit),
                    ("$offset", &offset),
                ],
            )
            .await?;
        Ok((rows.len(), normalize(&rows, divisor)))
    }
}

/// Seed the Seveso stations (idempotent) and return `sensor_id → station.id`.
pub async fn seed_stations(pool: &PgPool) -> Result<HashMap<String, i64>, sqlx::Error> {
    let mut map = HashMap::new();
    for s in SEVESO_STATIONS {
        let station = domain::upsert_station(
            pool,
            &NewStation {
                source: SOURCE.to_string(),
                external_id: s.sensor_id.to_string(),
                name: s.name.to_string(),
                river: "Seveso".to_string(),
                kind: StationKind::Hydrometric,
                lat: s.lat,
                lon: s.lon,
            },
        )
        .await?;
        map.insert(s.sensor_id.to_string(), station.id);
    }
    Ok(map)
}

/// Store observations on `station_id` under `metric`. Batched (one query per ≤1000 rows),
/// idempotent (upsert on `(station, metric, ts)`). Returns the number stored.
async fn store(
    pool: &PgPool,
    station_id: i64,
    metric: Metric,
    obs: &[ParsedObs],
) -> Result<usize, sqlx::Error> {
    let batch: Vec<(i64, OffsetDateTime, Metric, f64)> = obs
        .iter()
        .map(|o| (station_id, o.ts, metric, o.value))
        .collect();
    domain::upsert_observations(pool, &batch).await
}

/// One forward-poll cycle: seed stations, then store the recent level (and rain, where a
/// co-located gauge exists) for each.
pub async fn poll_once(pool: &PgPool, client: &ArpaClient) -> Result<usize, IngestError> {
    let map = seed_stations(pool).await?;
    let mut total = 0;
    for s in SEVESO_STATIONS {
        let station_id = map[s.sensor_id];
        let level = client.fetch_recent(s.sensor_id, 100, LEVEL_DIVISOR).await?;
        total += store(pool, station_id, Metric::LevelM, &level).await?;
        if let Some(rain_id) = s.rain_sensor_id {
            let rain = client.fetch_recent(rain_id, 100, RAIN_DIVISOR).await?;
            total += store(pool, station_id, Metric::RainMm, &rain).await?;
        }
    }
    Ok(total)
}

/// Page one dataset's full series for one sensor onto `station_id` under `metric`.
/// Terminates on a raw-empty page. Returns the number stored.
async fn backfill_sensor(
    pool: &PgPool,
    client: &ArpaClient,
    dataset: &str,
    sensor_id: &str,
    station_id: i64,
    metric: Metric,
    divisor: f64,
) -> Result<usize, IngestError> {
    let mut total = 0;
    let mut offset = 0;
    loop {
        let (raw, page) = client
            .fetch_page(dataset, sensor_id, offset, divisor)
            .await?;
        if raw == 0 {
            break; // no more rows (a page may be non-empty yet normalize to nothing, so
                   // terminate on the raw count, not `page`).
        }
        total += store(pool, station_id, metric, &page).await?;
        offset += PAGE;
        tracing::info!(
            sensor = sensor_id,
            dataset,
            ?metric,
            stored = total,
            "backfill progress"
        );
    }
    Ok(total)
}

/// One-shot historical backfill: page through **both** datasets (digitized history
/// 2021→2025, then the recent one 2025→now) for every station's level and co-located rain,
/// so the series is continuous up to the publication lag. Heavy (years of 10-min data) —
/// an operator step, not the poll path. Idempotent: re-running upserts the same points.
pub async fn backfill(pool: &PgPool, client: &ArpaClient) -> Result<usize, IngestError> {
    let map = seed_stations(pool).await?;
    let mut total = 0;
    for s in SEVESO_STATIONS {
        let station_id = map[s.sensor_id];
        for dataset in [DATASET_HISTORY, DATASET_LATEST] {
            total += backfill_sensor(
                pool,
                client,
                dataset,
                s.sensor_id,
                station_id,
                Metric::LevelM,
                LEVEL_DIVISOR,
            )
            .await?;
            if let Some(rain_id) = s.rain_sensor_id {
                total += backfill_sensor(
                    pool,
                    client,
                    dataset,
                    rain_id,
                    station_id,
                    Metric::RainMm,
                    RAIN_DIVISOR,
                )
                .await?;
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn row(idsensore: &str, data: &str, valore: &str) -> ArpaRow {
        ArpaRow {
            idsensore: idsensore.to_string(),
            data: data.to_string(),
            valore: valore.to_string(),
            stato: None,
        }
    }

    #[test]
    fn normalize_scales_level_cm_to_m_with_solar_offset() {
        let parsed = normalize(
            &[row("3118", "2026-06-03T00:30:00.000", "188")],
            LEVEL_DIVISOR,
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].value, 1.88); // 188 cm → 1.88 m
                                           // 00:30 in ARPA solar time (UTC+1) is the same instant as 23:30 UTC the day before.
        assert_eq!(parsed[0].ts.offset(), offset!(+1));
        assert_eq!(parsed[0].ts, datetime!(2026-06-02 23:30:00 UTC));
    }

    #[test]
    fn normalize_keeps_rain_mm_unscaled() {
        let parsed = normalize(
            &[row("8199", "2026-06-03T00:30:00.000", "12.4")],
            RAIN_DIVISOR,
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].value, 12.4); // mm kept as-is
    }

    #[test]
    fn normalize_drops_sentinel_and_unparseable() {
        let rows = [
            row("3118", "2026-06-03T00:30:00.000", "-9999"), // missing sentinel
            row("3118", "not-a-date", "12"),                 // bad timestamp
            row("3118", "2026-06-03T00:40:00.000", "abc"),   // bad value
            row("3118", "2026-06-03T00:50:00.000", "41"),    // good
        ];
        let parsed = normalize(&rows, LEVEL_DIVISOR);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].value, 0.41);
    }
}
