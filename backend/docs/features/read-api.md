# Feature: Read API (stations & observations)

## Overview
Public, read-only HTTP endpoints that expose stations and their observation time series to
the frontend. No auth (public data); writes/admin are gated in a later feature. All inputs
are validated and bounded.

## Design
- Handlers live in `src/api.rs`, mounted on the shared `Router` in `src/lib.rs`. They read
  through the `domain` repository (parameterized queries only) — no SQL in the API layer.
- **Serialization:** domain types (`Station`, `Observation`, `Threshold`) derive
  `serde::Serialize`; timestamps are RFC 3339 (`time::serde::rfc3339`); the `kind`/`level`/
  `metric` enums serialize as their string form.
- **Validation / bounds (SECURITY.md):** `limit` is clamped to `[1, 10000]` (default 1000);
  `from`/`to` are parsed as RFC 3339 and must satisfy `from <= to`; `metric` is an allow-list
  (`level_m` | `rain_mm`). Unknown station → 404; bad input → 400.
- **Error mapping:** `ApiError` → JSON `{ "error": ... }`. DB errors are logged and collapsed
  to a generic 500 — raw errors are never returned to clients.

## Files / code
- `src/api.rs` — `list_stations`, `get_station`, `station_observations`, `ApiError`, `StationDetail`.
- `src/lib.rs` — routes mounted on the router.
- `src/domain.rs` — `observations_in_range_limited` (bounded query backing the observations endpoint).
- `tests/read_api.rs` — `oneshot` integration tests against a seeded ephemeral DB.

## Contract
| Method | Path | Response |
|--------|------|----------|
| GET | `/stations` | `[Station, …]` |
| GET | `/stations/{id}` | `Station` + `thresholds: [Threshold, …]` (404 if unknown) |
| GET | `/stations/{id}/observations` | `[Observation, …]` ordered by `ts` asc |

Observations query params (all optional): `from`, `to` (RFC 3339; default = last 30 days),
`limit` (default 1000, max 10000), `metric` (`level_m` default | `rain_mm`).
Errors: `400` bad param / `from > to`; `404` unknown station; `500` generic (logged).

## Testing
`cargo test --features integration`: list/detail/404, observations range + `limit`, and
rejection of bad params (`from > to`, unknown metric, bad timestamp). Smoke-tested against
the real ingested Seveso data.

## Status
**Implemented.** Consumed by the frontend level chart (#6).

## Open questions
- Cursor/offset pagination if a single window can exceed 10000 points (10-min data ≈ 70 days).
- Downsampling/aggregation endpoint for long ranges (TimescaleDB continuous aggregates) later.
