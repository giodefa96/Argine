//! ARPA Lombardia hydrometry ingestion (Seveso).
//!
//! Source: Regione Lombardia open-data (Socrata). There is no public real-time API for
//! lowland river levels, so published values lag ~18h — see `DATA_SOURCES.md`. Two paths:
//! a scheduled forward poll (recent dataset) and a one-shot historical backfill (for the
//! frontend time-slider / ML). All HTTP is mocked with `wiremock` in tests — never hit the
//! real endpoint in CI (CLAUDE.md).

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

/// A Seveso hydrometric station to ingest. `sensor_id` is the ARPA `idsensore`, stored as
/// `station.external_id`. Coordinates are from the ARPA station registry (`nf78-nj6b`).
pub struct SeedStation {
    pub sensor_id: &'static str,
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

/// The active hydrometric stations on the Seveso (see `DATA_SOURCES.md`): upstream → city.
pub const SEVESO_STATIONS: &[SeedStation] = &[
    SeedStation {
        sensor_id: "8119",
        name: "Cantù Asnago",
        lat: 45.71853181,
        lon: 9.10037269,
    },
    SeedStation {
        sensor_id: "8121",
        name: "Paderno Dugnano Palazzolo",
        lat: 45.58280558,
        lon: 9.15897783,
    },
    SeedStation {
        sensor_id: "3118",
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
    idsensore: String,
    data: String,
    valore: String,
    #[serde(default)]
    #[allow(dead_code)] // kept for future quality filtering on the validity flag
    stato: Option<String>,
}

/// A normalized observation: level in **metres** at an absolute timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObs {
    pub sensor_id: String,
    pub ts: OffsetDateTime,
    pub value_m: f64,
}

/// Normalize raw ARPA rows → observations. Drops rows with an unparseable timestamp or
/// value, and the missing-value sentinel. Converts cm → m (ARPA reports level in cm).
fn normalize(rows: &[ArpaRow]) -> Vec<ParsedObs> {
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
            sensor_id: r.idsensore.clone(),
            ts: naive.assume_offset(ARPA_OFFSET),
            value_m: value / 100.0,
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

    /// The most recent `limit` observations for one sensor (forward poll).
    pub async fn fetch_recent(
        &self,
        sensor_id: &str,
        limit: u32,
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
        Ok(normalize(&rows))
    }

    /// One page of the historical series for a sensor, ordered ascending by time.
    async fn fetch_history_page(
        &self,
        sensor_id: &str,
        offset: u32,
    ) -> Result<Vec<ParsedObs>, reqwest::Error> {
        let (limit, offset) = (PAGE.to_string(), offset.to_string());
        let rows = self
            .get(
                DATASET_HISTORY,
                &[
                    ("idsensore", sensor_id),
                    ("$order", "data ASC"),
                    ("$limit", &limit),
                    ("$offset", &offset),
                ],
            )
            .await?;
        Ok(normalize(&rows))
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

/// Store observations, mapping `sensor_id → station.id`. Rows for unknown sensors are
/// skipped. Returns the number stored. Idempotent (upsert on `(station, metric, ts)`).
async fn store(
    pool: &PgPool,
    map: &HashMap<String, i64>,
    obs: &[ParsedObs],
) -> Result<usize, sqlx::Error> {
    let mut stored = 0;
    for o in obs {
        let Some(&station_id) = map.get(&o.sensor_id) else {
            continue;
        };
        domain::upsert_observation(pool, station_id, o.ts, Metric::LevelM, o.value_m).await?;
        stored += 1;
    }
    Ok(stored)
}

/// One forward-poll cycle: seed stations, then store the recent window for each.
pub async fn poll_once(pool: &PgPool, client: &ArpaClient) -> Result<usize, IngestError> {
    let map = seed_stations(pool).await?;
    let mut total = 0;
    for s in SEVESO_STATIONS {
        let obs = client.fetch_recent(s.sensor_id, 100).await?;
        total += store(pool, &map, &obs).await?;
    }
    Ok(total)
}

/// One-shot historical backfill: page through the history dataset for every station and
/// store it. Heavy (years of 10-min data) — an operator step, not the scheduled path.
pub async fn backfill(pool: &PgPool, client: &ArpaClient) -> Result<usize, IngestError> {
    let map = seed_stations(pool).await?;
    let mut total = 0;
    for s in SEVESO_STATIONS {
        let mut offset = 0;
        loop {
            let page = client.fetch_history_page(s.sensor_id, offset).await?;
            if page.is_empty() {
                break;
            }
            total += store(pool, &map, &page).await?;
            offset += PAGE;
            tracing::info!(sensor = s.sensor_id, stored = total, "backfill progress");
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
    fn normalize_parses_cm_to_m_with_solar_offset() {
        let parsed = normalize(&[row("3118", "2026-06-03T00:30:00.000", "188")]);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sensor_id, "3118");
        assert_eq!(parsed[0].value_m, 1.88); // 188 cm → 1.88 m
                                             // 00:30 in ARPA solar time (UTC+1) is the same instant as 23:30 UTC the day before.
        assert_eq!(parsed[0].ts.offset(), offset!(+1));
        assert_eq!(parsed[0].ts, datetime!(2026-06-02 23:30:00 UTC));
    }

    #[test]
    fn normalize_drops_sentinel_and_unparseable() {
        let rows = [
            row("3118", "2026-06-03T00:30:00.000", "-9999"), // missing sentinel
            row("3118", "not-a-date", "12"),                 // bad timestamp
            row("3118", "2026-06-03T00:40:00.000", "abc"),   // bad value
            row("3118", "2026-06-03T00:50:00.000", "41"),    // good
        ];
        let parsed = normalize(&rows);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].value_m, 0.41);
    }
}
