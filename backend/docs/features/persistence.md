# Feature: Persistence (PostgreSQL + TimescaleDB)

## Overview
The backend connects to a PostgreSQL database with the **TimescaleDB** extension, runs
versioned migrations at startup, and exposes a `/ready` readiness endpoint that reflects DB
connectivity. This is the foundation the domain model and ingestion build on.

## Design
- **Pool:** a single `sqlx::PgPool` (`PgPoolOptions`, max 5 connections) created at startup
  and shared via `AppState`.
- **Migrations:** SQL files under `backend/migrations/`, embedded at compile time by
  `sqlx::migrate!()` and applied on boot. Bootstrap migration (`0001_init.sql`) enables the
  TimescaleDB extension and creates a smoke table; the domain schema lands in the next feature.
- **Readiness vs liveness:** `/health` stays dependency-free (process liveness); `/ready`
  runs `SELECT 1` against the pool (can we serve real traffic?). Raw DB errors are logged,
  never returned to the client.
- **No TLS feature** on `sqlx`: local/compose DB connects over the internal network. Add a
  TLS feature when targeting a managed DB over the public internet.

## Files / code
- `Cargo.toml` — `sqlx` (runtime-tokio, postgres, macros, migrate); `integration` feature.
- `src/config.rs` — `Config::database_url`; `validate()` rejects a `changethis` DB password
  outside `local`.
- `src/lib.rs` — `AppState { pool }`; `router(state, config)`; `ready` handler.
- `src/main.rs` — build pool, `sqlx::migrate!().run(&pool)`, serve.
- `migrations/0001_init.sql` — TimescaleDB extension + `schema_smoke` table.
- `docker-compose.yml` (repo root) — `db` (TimescaleDB) + `backend`.

## Contract
- **Env:** `DATABASE_URL` (default for local: `postgres://argine:changethis@localhost:5432/argine`).
- **`GET /ready`** → `200 {"status":"ready"}` if the DB responds; `503 {"status":"unavailable"}` otherwise.

## Testing
- **Unit** (`src/config.rs`): default DB password rejected outside `local`.
- **DB-free integration** (`tests/health.rs`): `/health` via a lazy pool (no connection).
- **DB-backed integration** (`tests/ready.rs`, `#[cfg(feature = "integration")]`): `#[sqlx::test]`
  provisions an ephemeral DB per test and runs migrations; asserts `/ready` = 200 and that the
  bootstrap migration created `schema_smoke`. Run locally with `make db-up` then
  `DATABASE_URL=… cargo test --features integration`; CI runs it via `--all-features` with a
  TimescaleDB service container.

## Status
✅ Implemented (connection, migrations, `/ready`). Domain tables are intentionally out of scope.

## Open questions
- Connection pool sizing and timeouts for production load.
- When to add a `tls` feature (managed DB over public internet).
