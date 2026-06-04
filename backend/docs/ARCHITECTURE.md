# Backend architecture

Rust service built on **Axum** (see project-level [`IDEAS.md`](../../IDEAS.md) for the
product vision). This document tracks the *implemented* low-level structure; planned
components are marked as such.

## Startup flow (`src/main.rs`)
1. Initialize tracing (`tracing_subscriber::fmt` + `EnvFilter`, default `info`).
2. Load and validate configuration — `Config::from_env()` (`src/config.rs`). **Fails fast**
   if a secret is still the placeholder in non-local environments.
3. Connect the `PgPool` and run migrations (`sqlx::migrate!()`).
4. Build the Axum `Router` with `AppState`: routes + CORS layer + `TraceLayer`.
5. Bind `TcpListener` on `BIND_ADDR` and `axum::serve`.

## Modules
| Module | File | Responsibility |
|--------|------|----------------|
| entry  | `src/main.rs`   | wiring: tracing, pool, migrations, router, server |
| lib    | `src/lib.rs`    | `AppState`, `router()`, handlers |
| config | `src/config.rs` | env-driven config + startup validation |
| domain | `src/domain.rs` | entity types + thin repository (parameterized queries) |
| arpa   | `src/arpa.rs`   | ARPA hydrometry ingestion: client, normalize, poll, backfill |
| open_meteo | `src/open_meteo.rs` | Open-Meteo rain-forecast ingestion: client, normalize, poll |
| api    | `src/api.rs`    | public read endpoints (stations, observations) + input validation |

## HTTP surface (current)
| Method | Path      | Handler  | Description |
|--------|-----------|----------|-------------|
| GET    | `/health` | `health` | liveness probe → `{"status":"ok"}` (no dependencies) |
| GET    | `/ready`  | `ready`  | readiness → `200 {"status":"ready"}` if DB reachable, else `503` |
| GET    | `/stations` | `api::list_stations` | all stations |
| GET    | `/stations/{id}` | `api::get_station` | station + thresholds (404 if unknown) |
| GET    | `/stations/{id}/observations` | `api::station_observations` | series; `?from&to&limit&metric`, bounded |

See [`features/read-api.md`](./features/read-api.md) for the full contract and validation rules.

## Persistence
PostgreSQL + **TimescaleDB** via `sqlx::PgPool` (shared in `AppState`). Migrations in
`backend/migrations/`, embedded by `sqlx::migrate!()` and applied at startup. See
[`features/persistence.md`](./features/persistence.md).

### Domain schema
Core entities (migrations `0002_domain_model.sql`, `0003_weather_forecast.sql`).
`observation` and `weather_forecast` are TimescaleDB **hypertables** partitioned on `ts`;
their natural keys make ingestion idempotent. See
[`features/domain-model.md`](./features/domain-model.md) and
[`features/weather-ingestion.md`](./features/weather-ingestion.md).

```mermaid
erDiagram
    STATION ||--o{ THRESHOLD : "has"
    STATION ||--o{ OBSERVATION : "records"
    STATION ||--o{ WEATHER_FORECAST : "forecast at"
    STATION {
        bigint id PK
        text source "e.g. arpa_lombardia"
        text external_id "unique per source"
        text name
        text river
        text kind "hydrometric | rain"
        float lat
        float lon
        timestamptz created_at
    }
    THRESHOLD {
        bigint station_id FK
        text level "yellow | orange | red"
        float value_m
    }
    OBSERVATION {
        bigint station_id FK
        timestamptz ts "hypertable partition"
        text metric "level_m | rain_mm"
        float value
    }
    WEATHER_FORECAST {
        bigint station_id FK
        timestamptz run_ts "forecast issue time; every run kept"
        timestamptz ts "hypertable partition"
        float rain_mm
        text model "e.g. best_match"
    }
```

## Configuration
Read from environment (see [`features/config-and-startup.md`](./features/config-and-startup.md)):

| Var | Default | Notes |
|-----|---------|-------|
| `ENVIRONMENT` | `local` | `local` \| `staging` \| `production` |
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `SECRET_KEY` | `changethis` | **must** be changed in non-local envs (fail-fast) |
| `DATABASE_URL` | `postgres://argine:changethis@localhost:5432/argine` | DB connection; `changethis` password rejected outside `local` |
| `CORS_ORIGINS` | _(empty)_ | comma-separated explicit allowlist |

## Runtime / deploy
- Multi-stage `Dockerfile` → **distroless, non-root** runtime image (SECURITY.md §10).
- Dependency lockfile (`Cargo.lock`) committed; built with `--locked`.

## Testing
- **Unit tests** live next to the code (`#[cfg(test)]`); see `src/config.rs` for the
  security-critical validation tests.
- **Integration tests** in `tests/` exercise the router in-process via `tower`'s `oneshot`
  (no network bind) — see `tests/health.rs`.
- Run with `cargo test` (or `make test-backend`). Coverage via `cargo llvm-cov` in CI.
- External APIs (ARPA, Open-Meteo) must be mocked (`wiremock`) — never called in tests.

## Ingestion
Two background `tokio::time::interval` tasks (spawned in `main.rs`), both hourly, HTTP via
`reqwest` (rustls):
- **ARPA** poll → upserts `observation` rows — river **level** plus co-located **rainfall**
  (`rain_mm`) per station; `argine-backend backfill` runs a one-shot historical import. See
  [`features/arpa-ingestion.md`](./features/arpa-ingestion.md) and the data-source / latency
  notes in [`DATA_SOURCES.md`](../../DATA_SOURCES.md).
- **Open-Meteo** poll → stores one `weather_forecast` **run** per station per tick (keyed by
  `run_ts`; every run kept for backtesting). See
  [`features/weather-ingestion.md`](./features/weather-ingestion.md).

## Planned components (not yet implemented)
Tracked in [`IDEAS.md`](../../IDEAS.md); each will get a `features/` doc when built:
- Forecast engine: baseline (lag-based) → ML inference via **ONNX** (`ort`/`tract`).
- Alert engine + notification channels (SSE, Telegram, Web Push).
- Auth (JWT + Argon2) for admin/write endpoints.

## Startup sequence
```mermaid
flowchart TD
    A[env vars] --> B[Config::from_env]
    B -->|validate, fail-fast on default secrets| C{valid?}
    C -->|no| X[exit non-zero]
    C -->|yes| D[connect PgPool]
    D --> E["sqlx::migrate!().run()"]
    E --> F[build Router with AppState]
    F --> G["axum::serve(BIND_ADDR)"]
    F --> H["GET /health (liveness)"]
    F --> I["GET /ready (DB readiness)"]
    F --> J[CorsLayer allowlist + TraceLayer]
```

## Request → data flow (current)
```mermaid
flowchart LR
    client[Client] -->|GET /health| live[health handler]
    client -->|GET /ready| ready[ready handler]
    ready -->|SELECT 1| db[(PostgreSQL / TimescaleDB)]
    live --> client
    ready --> client
```
