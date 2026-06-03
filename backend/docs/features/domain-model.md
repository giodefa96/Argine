# Feature: Domain model (station, observation, threshold)

## Overview
The core persisted entities the rest of the system builds on: measurement **stations**, their
Civil-Protection alert **thresholds**, and the **observation** time series (river level / rain).
This feature defines the schema; ingestion and the read API consume it in later features.

## Design
- **`station`** — one row per physical gauge, identified within its data provider by
  `(source, external_id)` (unique). `kind` is `hydrometric` or `rain`; coordinates are
  bounded by `CHECK` (lat ∈ [-90,90], lon ∈ [-180,180]) so bad geo data can't land.
- **`threshold`** — alert levels per station, primary key `(station_id, level)` so a station
  has at most one value per `yellow|orange|red`. `ON DELETE CASCADE` with the station.
- **`observation`** — TimescaleDB **hypertable** partitioned on `ts`. No surrogate key: the
  natural key `(station_id, metric, ts)` is `UNIQUE`, which (a) makes re-ingestion idempotent
  via `ON CONFLICT DO UPDATE` and (b) includes the partition column `ts`, satisfying
  TimescaleDB's rule that a unique index must cover it. Extra index `(station_id, ts DESC)`
  supports the planned "one station's series over a time range" read query.
- **Why `CHECK` over Postgres `ENUM`:** enums are painful to extend in migrations; a `CHECK`
  on `text` gives the same guarantee and evolves with a plain `ALTER`.

## Files / code
- `migrations/0002_domain_model.sql` — `station`, `threshold`, `observation` (+ `create_hypertable`).
- `src/domain.rs` — typed entities (`Station`, `Threshold`, `Observation`, `NewStation`) and the
  `TEXT`-backed enums `StationKind` / `AlertLevel` / `Metric` (a `text_enum!` macro derives their
  `sqlx` `Type`/`Encode`/`Decode`). Thin repository: `upsert_station`, `get_station`,
  `list_stations`, `upsert_threshold`, `thresholds_for_station`, `upsert_observation`,
  `observations_in_range` — all parameterized (`.bind`), idempotent upserts via `ON CONFLICT`.
- `tests/repo.rs` — DB-backed CRUD tests (`#[sqlx::test]`, `integration` feature).

## Contract
- **`station`**: `id`, `source`, `external_id`, `name`, `river`, `kind`, `lat`, `lon`, `created_at`.
- **`threshold`**: `station_id`, `level`, `value_m`.
- **`observation`**: `station_id`, `ts`, `metric`, `value`.
- Units: level in metres (`level_m`), rain in millimetres (`rain_mm`); thresholds in metres.

## Testing
- The migration is exercised end-to-end by the DB-backed integration tests, which apply
  **all** migrations on an ephemeral DB. Verified locally against
  `timescale/timescaledb:2.17.2-pg16`: `observation` is a hypertable, all `CHECK` constraints
  and FKs present.
- `tests/repo.rs` covers: station upsert idempotency on `(source, external_id)`, get/list,
  threshold upsert + per-station listing, observation upsert idempotency on
  `(station, metric, ts)`, and time-range filtering.

## Status
**Implemented.** Schema (`0002`) + typed entities + thin repository + CRUD tests, all green.
Consumed by ARPA ingestion (#3) and the read API (#5) in later features.

## Open questions
- Drop the now-superseded `schema_smoke` table (from `0001`) and repoint
  `tests/ready.rs::migrations_created_the_smoke_table` at a real table?
- Add `updated_at` / soft-delete on `station`, or keep it append-mostly for the MVP?
- Continuous aggregates / retention policy on `observation` — defer until ingestion volume is known.
