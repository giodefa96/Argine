# Feature: Open-Meteo rain-forecast ingestion

## Overview

Pulls the hourly **forecast precipitation** for the Seveso basin from Open-Meteo and stores it
as `weather_forecast` rows. This is the forward-looking counterpart of the observed rainfall
from ARPA (`metric = rain_mm`), and the main input for the planned rainfall → level forecast
(IDEAS.md §6).

## Design

- **Source:** [Open-Meteo](https://open-meteo.com) forecast API — hourly precipitation by
  coordinates, free, **no API key** (see `DATA_SOURCES.md`). Real-time, unlike the ~18 h-delayed
  ARPA feed.
- **Basin as points (MVP):** the basin is approximated by the coordinates of the three Seveso
  stations (`arpa::SEVESO_STATIONS`, reused — no new seed list), answering IDEAS.md §12 Q3 with
  "a set of points is enough for now". A real basin entity/boundary can replace `station_id`
  later without touching ingestion.
- **Every run is kept:** a poll stores a full forecast *run* keyed by `run_ts` (the fetch time)
  under model `best_match`. Runs are never overwritten by newer ones — the history of *what was
  forecast when* is exactly what backtesting and ML training need (IDEAS.md §7 draft schema
  `weather_forecast(…, ts, run_ts, rain_mm, model)`). Re-ingesting the **same** run upserts in
  place (idempotent on `(station, model, run_ts, ts)`).
- **Normalization:** with `timezone=UTC` the API returns naive ISO 8601 hours in UTC;
  `normalize` zips `hourly.time` with `hourly.precipitation`, drops `null` values and
  unparseable timestamps, and attaches the UTC offset. Horizon: `forecast_days=3` (the basin's
  concentration time is hours; 3 days comfortably covers the useful window).
- **Scheduler:** a second `tokio::time::interval` background task in `main.rs`, hourly like the
  ARPA poll (Open-Meteo refreshes its models on a similar cadence). `run_ts` is computed by the
  caller (`now_utc()` in `main`, fixed values in tests) so polls are deterministic to test.

## Files / code

- `src/open_meteo.rs` — `OpenMeteoClient` (reqwest + rustls), `normalize`, `poll_once`,
  `DEFAULT_BASE_URL`, `MODEL`, `IngestError`.
- `src/domain.rs` — `WeatherForecast`, `upsert_weather_forecasts` (batched ≤1000/query),
  `latest_weather_forecast` (all points of a station's most recent run).
- `migrations/0003_weather_forecast.sql` — `weather_forecast` hypertable, unique on
  `(station_id, model, run_ts, ts)`, index for "latest run per station".
- `src/main.rs` — spawns the hourly forecast-poll task alongside the ARPA poll.
- `tests/open_meteo.rs` — wiremock + `#[sqlx::test]`: a poll stores one run per station and is
  idempotent; a newer run is kept alongside the old and becomes "latest". Unit tests in
  `open_meteo.rs` cover `normalize` (UTC parsing, null/bad-timestamp drops, zip truncation).

## Contract

- **Endpoint:** `GET {base}/forecast?latitude&longitude&hourly=precipitation&forecast_days=3&timezone=UTC`.
  Base URL `DEFAULT_BASE_URL` (`https://api.open-meteo.com/v1`), overridden in tests.
- **Stored:** `weather_forecast(station_id, run_ts, ts, rain_mm, model)` — mm per hour, UTC.
- **No secrets / no auth:** public API; HTTP failures are logged, the poll loop continues.

## Testing

`cargo test --features integration` (needs a reachable DB). External HTTP is **always** mocked
with `wiremock` — the real Open-Meteo endpoint is never called in CI (CLAUDE.md).

## Status

**Implemented** for the forecast API at the three station points.

## Open questions

- **ERA5 archive** (historical *observed* reanalysis rain, `archive-api.open-meteo.com`) is not
  ingested yet — useful as ML training input where ARPA gauges have gaps.
- Retention/aggregation policy for old runs once volume grows (hypertable makes either cheap).
- Specific models (`icon_seamless`, `ecmwf_ifs025`, …) instead of `best_match`, and storing
  more variables (soil moisture is a candidate forecast feature, IDEAS.md §6).
