# Feature: ARPA hydrometry ingestion (Seveso)

## Overview
Pulls Seveso river level from ARPA Lombardia and stores it as `observation` rows. Two paths:
a **scheduled forward poll** (recent values, runs in the background alongside the HTTP server)
and a **one-shot historical backfill** (years of data, for the frontend time-slider / ML training).

## Design
- **Source:** Regione Lombardia open-data (Socrata REST/JSON), licence IODL 2.0 — see
  [`DATA_SOURCES.md`](../../../DATA_SOURCES.md). There is **no public real-time API** for lowland
  river levels: the open-data feed for the MI/MB/CO/LC/VA network is published with a **~18 h batch
  delay**, so the poll cadence is hourly (polling faster gains nothing).
- **Stations:** the 3 active Seveso hydrometric stations are a compile-time constant
  (`SEVESO_STATIONS`): Cantù Asnago (8119), Paderno Dugnano Palazzolo (8121), Milano Niguarda (3118).
  Seeded idempotently (`upsert_station`) with `source = "arpa_lombardia"`, `external_id = idsensore`.
- **Normalization:** ARPA reports level in **cm** at timestamps in **solar time (UTC+1)** with no
  offset. `normalize()` converts cm → m, attaches the +1 offset, and drops rows with an unparseable
  timestamp/value or the missing-value sentinel (≤ −900). Sensor id maps to our `station.id` via the
  seed map; observations upsert on `(station, metric, ts)` so re-ingestion is idempotent.
- **Scheduler:** a plain `tokio::time::interval` background task (not `tokio-cron-scheduler`) —
  hourly cadence needs nothing more; one fewer dependency. Swap in cron later if needed.
- **Backfill:** pages the history dataset (`$limit`/`$offset`, 1000/page) per station until a page
  comes back empty. Heavy (years of 10-min data) → an operator command, not the scheduled path.

## Files / code
- `src/arpa.rs` — `ArpaClient` (reqwest + rustls), `normalize`, `seed_stations`, `store`,
  `poll_once`, `backfill`, `SEVESO_STATIONS`, `IngestError`.
- `src/main.rs` — spawns the hourly poll task; `argine-backend backfill` runs the one-shot import.
- `tests/arpa.rs` — wiremock + `#[sqlx::test]`: poll seeds stations + stores + is idempotent;
  backfill pages until empty. Unit tests in `arpa.rs` cover `normalize` (cm→m, offset, drops).

## Contract
- **Datasets:** `647i-nhxk` (recent), `3e8b-w7ay` (history 2021→2025). Base URL `DEFAULT_BASE_URL`.
- **Run modes:** default = server + hourly poll; `argine-backend backfill` = import history then exit.
- **Stored:** `observation(station_id, ts, metric = level_m, value)` with `value` in metres.
- **No secrets / no auth:** public open-data; HTTP failures are logged, the poll loop continues.

## Testing
`cargo test --features integration` (needs a reachable DB). External HTTP is **always** mocked
with `wiremock` — the real ARPA endpoint is never called in CI (CLAUDE.md).

## Status
**Implemented** for ARPA hydrometry (level). Open-Meteo rain ingestion is the next feature (#4).

## Open questions
- Use the ARPA `stato` validity flag to filter non-validated readings? (Currently only the sentinel
  is dropped; `stato` is parsed but unused.)
- Real-time: open-data lags ~18 h for the lowland network. A faster feed needs a direct ARPA
  arrangement (see DATA_SOURCES.md) before live alerting is meaningful.
- Backfill spans two datasets (history ends ~2025-01, recent covers 2025→now); current backfill
  reads the history dataset only — wire the recent dataset's full range too when needed.
