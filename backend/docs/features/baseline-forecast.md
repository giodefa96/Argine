# Feature: baseline level forecast

## Overview

The first **rainfall → level** forecast (Phase 1, IDEAS.md §6): a transparent empirical model
in pure Rust, no ML. Exposed as `GET /stations/{id}/forecast` — predicted level for the next
1–48 hours, computed on the fly from the latest observed level and the latest Open-Meteo run.
ML (Phase 2) will replace the math behind the same interface.

## Design

- **Model** (the exact form from IDEAS.md §6):
  `level(t+h) = level(t) + α · cum_forecast_rain(t..t+h) − β · h`
  — current level, plus a linear rain response on the cumulative forecast rain at the station
  point, minus a linear drainage term. Implemented as the pure function
  `forecast::baseline(level0, rain_mm_per_hour, α, β)`.
- **α / β are uncalibrated placeholders** (`ALPHA_M_PER_MM = 0.02`, `BETA_M_PER_H = 0.01`):
  fitting them on historical events is the Phase 2 backtesting job. The model identifier is
  `baseline_v0` so clients/storage can distinguish versions.
- **Anchor = "now":** the latest observed level (stale by ARPA's ~18 h publication lag —
  surfaced via `based_on.level_ts` so clients can judge staleness) plus the future hours
  (`ts > now`) of the station's latest `weather_forecast` run.
- **Computed on demand, nothing stored.** The `level_forecast` table from the IDEAS.md §7
  draft arrives when the alert engine needs persisted/auditable predictions.
- **Invariant (property-tested):** more rain must **never lower** the predicted level — holds
  by construction (α ≥ 0) and is guarded by `proptest` against regressions, since the alert
  engine will sit on top of this output.
- **No data ≠ error:** a station without a level observation or weather run returns `200`
  with `points: []` and `based_on: null` — the frontend just has nothing to draw yet.

## Files / code

- `src/forecast.rs` — `baseline` (pure), `for_station` (assembles inputs), `StationForecast`/
  `ForecastPoint`/`BasedOn`, `MODEL_VERSION`, `ALPHA_M_PER_MM`, `BETA_M_PER_H`.
- `src/domain.rs` — `latest_observation` (freshness anchor).
- `src/api.rs` — `station_forecast` handler (+ horizon validation); route in `src/lib.rs`.
- `tests/forecast.rs` — `oneshot` + `#[sqlx::test]`: math end-to-end, empty-data shape,
  horizon cap and param validation. Unit + `proptest` tests in `forecast.rs`.

## Contract

- **`GET /stations/{id}/forecast?hours`** — `hours` defaults to 12, validated to `1..=48`
  (400 outside; 404 for an unknown station).
- **Response:** `{ station_id, model: "baseline_v0", based_on: { level_ts, level_m,
  weather_run_ts } | null, points: [{ ts, value_m }] }`, timestamps RFC 3339.
- Public and read-only, like the rest of the read API; DB errors are logged, never leaked.

## Testing

`cargo test` runs the pure-model unit + property tests DB-free;
`cargo test --features integration` adds the endpoint tests (seeded ephemeral DB, no HTTP
mocks needed). New dev-dependency: `proptest` (MIT/Apache-2.0).

## Status

**Implemented** (`baseline_v0`, uncalibrated).

## Open questions

- **Calibration:** fit α (and a per-station variant) and β on historical events; add the
  upstream→downstream lag explicitly (rain at Cantù reaches Niguarda hours later — the
  cumulative form only approximates this).
- Mix **observed** recent rain (last 6–24 h) into the anchor, not just forecast rain.
- Persist predictions (`level_forecast`) once the alert engine needs auditable runs.
