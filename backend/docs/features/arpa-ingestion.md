# Feature: ARPA ingestion (Seveso) — level + rainfall

## Overview

Pulls Seveso river **level** and co-located **rainfall** from ARPA Lombardia and stores them as
`observation` rows. Two paths: a **scheduled forward poll** (recent values, runs in the
background alongside the HTTP server) and a **one-shot historical backfill** (years of data, for
the frontend time-slider / ML training).

## Design

- **Source:** Regione Lombardia open-data (Socrata REST/JSON), licence IODL 2.0 — see
  [`DATA_SOURCES.md`](../../../DATA_SOURCES.md). There is **no public real-time API** for lowland
  river levels: the open-data feed for the MI/MB/CO/LC/VA network is published with a **~18 h batch
  delay**, so the poll cadence is hourly (polling faster gains nothing).
- **Stations + rain:** the 3 active Seveso hydrometric stations are a compile-time constant
  (`SEVESO_STATIONS`): Cantù Asnago (8119), Paderno Dugnano Palazzolo (8121), Milano Niguarda
  (3118). Each is paired with a co-located rain gauge (`rain_sensor_id`: 8199, 30525, 4065) whose
  rainfall is stored **on the same station** as `metric = rain_mm` — so level and rain correlate
  at one place. Seeded idempotently (`upsert_station`), `external_id = hydrometric idsensore`.
- **Normalization:** ARPA reports level in **cm** and rain in **mm**, at timestamps in **solar
  time (UTC+1)** with no offset. `normalize(rows, divisor)` divides by 100 for level (cm→m) or 1
  for rain, attaches the +1 offset, and drops rows with an unparseable timestamp/value or the
  missing-value sentinel (≤ −900). A divisor (not a ×0.01 factor) keeps values like 188→1.88 exact.
- **Storage:** observations upsert on `(station, metric, ts)` so re-ingestion is idempotent;
  batched via `domain::upsert_observations` (one query per ≤1000 rows).
- **Scheduler:** a plain `tokio::time::interval` background task (not `tokio-cron-scheduler`) —
  hourly cadence needs nothing more; one fewer dependency. Swap in cron later if needed.
- **Backfill:** pages the history dataset (`$limit`/`$offset`, 1000/page) per sensor until a
  **raw-empty** page (a full page can normalize to nothing during an all-sentinel outage, so it
  terminates on the raw row count, not the normalized result). Both level and rain are backfilled.

## Files / code

- `src/arpa.rs` — `ArpaClient` (reqwest + rustls), `normalize`, `seed_stations`, `store`,
  `poll_once`, `backfill`/`backfill_sensor`, `SEVESO_STATIONS` (with `rain_sensor_id`), `IngestError`.
- `src/main.rs` — spawns the hourly poll task; `argine-backend backfill` runs the one-shot import.
- `tests/arpa.rs` — wiremock + `#[sqlx::test]`: poll stores level + rain on the same station and
  is idempotent; backfill pages past a sentinel-only page until raw-empty, for both metrics. Unit
  tests in `arpa.rs` cover `normalize` (cm→m, rain mm kept, offset, drops).

## Contract

- **Datasets:** `647i-nhxk` (recent), `3e8b-w7ay` (history 2021→2025). Base URL `DEFAULT_BASE_URL`.
- **Run modes:** default = server + hourly poll; `argine-backend backfill` = import history then exit.
- **Stored:** `observation(station_id, ts, metric, value)` — `level_m` (metres) and `rain_mm` (mm).
- **No secrets / no auth:** public open-data; HTTP failures are logged, the poll loop continues.

## Testing

`cargo test --features integration` (needs a reachable DB). External HTTP is **always** mocked
with `wiremock` — the real ARPA endpoint is never called in CI (CLAUDE.md).

## Status

**Implemented** for ARPA level + co-located rainfall. Open-Meteo (forecast + ERA5 archive) rain is
the next feature.

## Open questions

- Use the ARPA `stato` validity flag to filter non-validated readings? (Currently only the sentinel
  is dropped; `stato` is parsed but unused.)
- Real-time: open-data lags ~18 h for the lowland network. A faster feed needs a direct ARPA
  arrangement (see DATA_SOURCES.md) before live alerting is meaningful.
- Backfill reads the history dataset only (ends ~2025-01); wire the recent dataset's full range too
  when needed. Niguarda's rain gauge (4065 Cinisello) is nearby, not exactly co-located.
