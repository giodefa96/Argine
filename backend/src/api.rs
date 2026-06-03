//! Public read API: stations and their observation time series.
//!
//! Read-only and unauthenticated — this is public data (levels). Writes/admin will be gated
//! later. Inputs are validated and bounded (pagination, time range); raw DB errors are
//! logged and never leaked to clients (SECURITY.md).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::domain::{self, Metric, Observation, Station, Threshold};
use crate::AppState;

const DEFAULT_LIMIT: i64 = 1000;
const MAX_LIMIT: i64 = 10_000;
const DEFAULT_WINDOW_DAYS: i64 = 30;

/// Errors surfaced as JSON. DB errors are logged and collapsed to a generic 500.
pub enum ApiError {
    NotFound,
    BadRequest(&'static str),
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found"),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::warn!(error = %e, "database error in read API");
        ApiError::Internal
    }
}

/// `GET /stations` — all stations.
pub async fn list_stations(State(s): State<AppState>) -> Result<Json<Vec<Station>>, ApiError> {
    Ok(Json(domain::list_stations(&s.pool).await?))
}

/// A station plus its alert thresholds.
#[derive(serde::Serialize)]
pub struct StationDetail {
    #[serde(flatten)]
    station: Station,
    thresholds: Vec<Threshold>,
}

/// `GET /stations/{id}` — one station with its thresholds; 404 if unknown.
pub async fn get_station(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<StationDetail>, ApiError> {
    let station = domain::get_station(&s.pool, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let thresholds = domain::thresholds_for_station(&s.pool, id).await?;
    Ok(Json(StationDetail {
        station,
        thresholds,
    }))
}

/// Query params for the observations endpoint (all optional).
#[derive(Deserialize)]
pub struct ObsQuery {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
    metric: Option<String>,
}

fn parse_ts(s: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(s, &Rfc3339)
        .map_err(|_| ApiError::BadRequest("invalid timestamp (RFC 3339)"))
}

/// `GET /stations/{id}/observations?from&to&limit&metric` — a station's series.
///
/// Defaults: `metric=level_m`, last 30 days, `limit=1000` (capped at 10000). Times are
/// RFC 3339. 404 if the station is unknown, 400 on a bad param or `from > to`.
pub async fn station_observations(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<ObsQuery>,
) -> Result<Json<Vec<Observation>>, ApiError> {
    if domain::get_station(&s.pool, id).await?.is_none() {
        return Err(ApiError::NotFound);
    }

    let metric = match q.metric.as_deref() {
        None | Some("level_m") => Metric::LevelM,
        Some("rain_mm") => Metric::RainMm,
        Some(_) => return Err(ApiError::BadRequest("invalid metric")),
    };

    let to = match q.to {
        Some(ref s) => parse_ts(s)?,
        None => OffsetDateTime::now_utc(),
    };
    let from = match q.from {
        Some(ref s) => parse_ts(s)?,
        None => to - Duration::days(DEFAULT_WINDOW_DAYS),
    };
    if from > to {
        return Err(ApiError::BadRequest("from must be <= to"));
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let obs = domain::observations_in_range_limited(&s.pool, id, metric, from, to, limit).await?;
    Ok(Json(obs))
}
