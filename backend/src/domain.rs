//! Domain model and thin repository layer.
//!
//! Maps the tables in `migrations/0002_domain_model.sql`. Every query is parameterized
//! (`.bind(..)`, never string-formatted SQL) per SECURITY.md. The `kind`/`level`/`metric`
//! text columns (DB-side `CHECK`-constrained) are surfaced as typed Rust enums.

use sqlx::PgPool;
use time::OffsetDateTime;

/// Returned when a text column holds a value outside the domain enum's known set
/// (should be impossible given the DB `CHECK` constraints, but decoded defensively).
#[derive(Debug, thiserror::Error)]
#[error("invalid {kind} value: {value:?}")]
pub struct ParseEnumError {
    kind: &'static str,
    value: String,
}

/// Generates a `TEXT`-backed enum: the variants, `as_str`/`FromStr`, and the `sqlx`
/// `Type`/`Encode`/`Decode` impls that make it bind and decode exactly like a `&str`.
macro_rules! text_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = ParseEnumError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant),)+
                    other => Err(ParseEnumError {
                        kind: stringify!($name),
                        value: other.to_string(),
                    }),
                }
            }
        }

        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <str as sqlx::Type<sqlx::Postgres>>::type_info()
            }
            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <&'r str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
                Ok(s.parse()?)
            }
        }

        impl<'q> sqlx::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
                <&'q str as sqlx::Encode<'q, sqlx::Postgres>>::encode(self.as_str(), buf)
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }
    };
}

text_enum!(
    /// Whether a station measures river level or rainfall.
    StationKind { Hydrometric => "hydrometric", Rain => "rain" }
);
text_enum!(
    /// Civil-Protection alert severity.
    AlertLevel { Yellow => "yellow", Orange => "orange", Red => "red" }
);
text_enum!(
    /// What an observation measures.
    Metric { LevelM => "level_m", RainMm => "rain_mm" }
);

/// A measurement station as stored (with its generated `id` and `created_at`).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Station {
    pub id: i64,
    pub source: String,
    pub external_id: String,
    pub name: String,
    pub river: String,
    pub kind: StationKind,
    pub lat: f64,
    pub lon: f64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// Fields needed to create (or upsert) a station — no `id`/`created_at`.
#[derive(Debug, Clone)]
pub struct NewStation {
    pub source: String,
    pub external_id: String,
    pub name: String,
    pub river: String,
    pub kind: StationKind,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Threshold {
    pub station_id: i64,
    pub level: AlertLevel,
    pub value_m: f64,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Observation {
    pub station_id: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub metric: Metric,
    pub value: f64,
}

/// One forecast point: rain expected at `ts`, as issued by `model` at `run_ts`.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct WeatherForecast {
    pub station_id: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub run_ts: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub ts: OffsetDateTime,
    pub rain_mm: f64,
    pub model: String,
}

/// Insert a station, or update it in place if `(source, external_id)` already exists.
/// Idempotent — ingestion can call this on every poll without creating duplicates.
pub async fn upsert_station(pool: &PgPool, s: &NewStation) -> sqlx::Result<Station> {
    sqlx::query_as::<_, Station>(
        r#"
        INSERT INTO station (source, external_id, name, river, kind, lat, lon)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (source, external_id) DO UPDATE
        SET name = EXCLUDED.name, river = EXCLUDED.river, kind = EXCLUDED.kind,
            lat = EXCLUDED.lat, lon = EXCLUDED.lon
        RETURNING id, source, external_id, name, river, kind, lat, lon, created_at
        "#,
    )
    .bind(&s.source)
    .bind(&s.external_id)
    .bind(&s.name)
    .bind(&s.river)
    .bind(s.kind)
    .bind(s.lat)
    .bind(s.lon)
    .fetch_one(pool)
    .await
}

pub async fn get_station(pool: &PgPool, id: i64) -> sqlx::Result<Option<Station>> {
    sqlx::query_as::<_, Station>(
        "SELECT id, source, external_id, name, river, kind, lat, lon, created_at
         FROM station WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_stations(pool: &PgPool) -> sqlx::Result<Vec<Station>> {
    sqlx::query_as::<_, Station>(
        "SELECT id, source, external_id, name, river, kind, lat, lon, created_at
         FROM station ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

/// Set the threshold for a `(station, level)` pair, updating the value if it exists.
pub async fn upsert_threshold(
    pool: &PgPool,
    station_id: i64,
    level: AlertLevel,
    value_m: f64,
) -> sqlx::Result<Threshold> {
    sqlx::query_as::<_, Threshold>(
        r#"
        INSERT INTO threshold (station_id, level, value_m)
        VALUES ($1, $2, $3)
        ON CONFLICT (station_id, level) DO UPDATE SET value_m = EXCLUDED.value_m
        RETURNING station_id, level, value_m
        "#,
    )
    .bind(station_id)
    .bind(level)
    .bind(value_m)
    .fetch_one(pool)
    .await
}

pub async fn thresholds_for_station(
    pool: &PgPool,
    station_id: i64,
) -> sqlx::Result<Vec<Threshold>> {
    sqlx::query_as::<_, Threshold>(
        "SELECT station_id, level, value_m FROM threshold WHERE station_id = $1 ORDER BY value_m",
    )
    .bind(station_id)
    .fetch_all(pool)
    .await
}

/// Store an observation, overwriting the value if one already exists for the same
/// `(station, metric, ts)` — idempotent re-ingestion of a reported point.
pub async fn upsert_observation(
    pool: &PgPool,
    station_id: i64,
    ts: OffsetDateTime,
    metric: Metric,
    value: f64,
) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO observation (station_id, ts, metric, value)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (station_id, metric, ts) DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(station_id)
    .bind(ts)
    .bind(metric)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Batch-upsert observations in chunks of 1000 — far fewer round-trips than one row at a
/// time, which matters for the backfill (years of 10-min data per station). Same idempotent
/// `ON CONFLICT` as [`upsert_observation`]. Returns the number of input rows.
pub async fn upsert_observations(
    pool: &PgPool,
    rows: &[(i64, OffsetDateTime, Metric, f64)],
) -> sqlx::Result<usize> {
    for chunk in rows.chunks(1000) {
        let mut qb =
            sqlx::QueryBuilder::new("INSERT INTO observation (station_id, ts, metric, value) ");
        qb.push_values(chunk, |mut b, (station_id, ts, metric, value)| {
            b.push_bind(*station_id)
                .push_bind(*ts)
                .push_bind(*metric)
                .push_bind(*value);
        });
        qb.push(" ON CONFLICT (station_id, metric, ts) DO UPDATE SET value = EXCLUDED.value");
        qb.build().execute(pool).await?;
    }
    Ok(rows.len())
}

/// The most recent observation of one metric at a station, if any. The freshness anchor
/// for the baseline forecast (the ARPA level lags ~18 h — see DATA_SOURCES.md).
pub async fn latest_observation(
    pool: &PgPool,
    station_id: i64,
    metric: Metric,
) -> sqlx::Result<Option<Observation>> {
    sqlx::query_as::<_, Observation>(
        "SELECT station_id, ts, metric, value FROM observation
         WHERE station_id = $1 AND metric = $2 ORDER BY ts DESC LIMIT 1",
    )
    .bind(station_id)
    .bind(metric)
    .fetch_optional(pool)
    .await
}

/// Batch-upsert forecast points for one `(station, model, run_ts)` run, chunked like
/// [`upsert_observations`]. Idempotent on `(station, model, run_ts, ts)` — re-ingesting
/// the same run overwrites it; a new `run_ts` is kept as a separate run (for backtesting).
/// Returns the number of input rows.
pub async fn upsert_weather_forecasts(
    pool: &PgPool,
    station_id: i64,
    model: &str,
    run_ts: OffsetDateTime,
    points: &[(OffsetDateTime, f64)],
) -> sqlx::Result<usize> {
    for chunk in points.chunks(1000) {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO weather_forecast (station_id, run_ts, ts, rain_mm, model) ",
        );
        qb.push_values(chunk, |mut b, (ts, rain_mm)| {
            b.push_bind(station_id)
                .push_bind(run_ts)
                .push_bind(*ts)
                .push_bind(*rain_mm)
                .push_bind(model);
        });
        qb.push(
            " ON CONFLICT (station_id, model, run_ts, ts)
              DO UPDATE SET rain_mm = EXCLUDED.rain_mm",
        );
        qb.build().execute(pool).await?;
    }
    Ok(points.len())
}

/// The latest forecast run for a station: every point of the most recent `run_ts`
/// (any model), ordered by forecast hour. Empty if no run has been ingested yet.
pub async fn latest_weather_forecast(
    pool: &PgPool,
    station_id: i64,
) -> sqlx::Result<Vec<WeatherForecast>> {
    sqlx::query_as::<_, WeatherForecast>(
        r#"
        SELECT station_id, run_ts, ts, rain_mm, model FROM weather_forecast
        WHERE station_id = $1
          AND run_ts = (SELECT max(run_ts) FROM weather_forecast WHERE station_id = $1)
        ORDER BY ts
        "#,
    )
    .bind(station_id)
    .fetch_all(pool)
    .await
}

/// A station's series for one metric over `[from, to]`, ordered by time. Capped at a
/// large bound; the read API uses [`observations_in_range_limited`] for explicit paging.
pub async fn observations_in_range(
    pool: &PgPool,
    station_id: i64,
    metric: Metric,
    from: OffsetDateTime,
    to: OffsetDateTime,
) -> sqlx::Result<Vec<Observation>> {
    observations_in_range_limited(pool, station_id, metric, from, to, 100_000).await
}

/// Bucketed series for long ranges (TimescaleDB `time_bucket`): one point per `bucket`
/// (a Postgres interval string from the API's allowlist, e.g. "1 hour" — never raw user
/// input). Level (instantaneous) is **averaged**; rain (cumulative) is **summed**, so a
/// day bucket reads as "total rain that day". `ts` is the bucket start.
pub async fn observations_bucketed(
    pool: &PgPool,
    station_id: i64,
    metric: Metric,
    from: OffsetDateTime,
    to: OffsetDateTime,
    bucket: &str,
    limit: i64,
) -> sqlx::Result<Vec<Observation>> {
    sqlx::query_as::<_, Observation>(
        r#"
        SELECT station_id, time_bucket($5::interval, ts) AS ts, metric,
               CASE WHEN metric = 'rain_mm' THEN sum(value) ELSE avg(value) END AS value
        FROM observation
        WHERE station_id = $1 AND metric = $2 AND ts >= $3 AND ts <= $4
        GROUP BY station_id, time_bucket($5::interval, ts), metric
        ORDER BY ts LIMIT $6
        "#,
    )
    .bind(station_id)
    .bind(metric)
    .bind(from)
    .bind(to)
    .bind(bucket)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// As [`observations_in_range`] but with an explicit row cap (bounded pagination for the
/// API). When the window holds more than `limit` points, the **most recent** ones win
/// (inner DESC limit, outer ASC) — a chart of "the last N days" must never lose its most
/// recent tail to truncation (sensor cadence varies: Niguarda publishes every 5 minutes,
/// the others every 10).
pub async fn observations_in_range_limited(
    pool: &PgPool,
    station_id: i64,
    metric: Metric,
    from: OffsetDateTime,
    to: OffsetDateTime,
    limit: i64,
) -> sqlx::Result<Vec<Observation>> {
    sqlx::query_as::<_, Observation>(
        r#"
        SELECT station_id, ts, metric, value FROM (
            SELECT station_id, ts, metric, value FROM observation
            WHERE station_id = $1 AND metric = $2 AND ts >= $3 AND ts <= $4
            ORDER BY ts DESC LIMIT $5
        ) latest ORDER BY ts
        "#,
    )
    .bind(station_id)
    .bind(metric)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(pool)
    .await
}
