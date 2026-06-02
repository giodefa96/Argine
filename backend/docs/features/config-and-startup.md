# Feature: Configuration & startup validation

## Overview
The backend reads all configuration from environment variables at startup and refuses to
run with insecure defaults in non-local environments. This is the first line of the
security baseline (SECURITY.md §3).

## Design
- A single `Config` struct is built by `Config::from_env()`, then `validate()`d before the
  server starts. Invalid config aborts startup with a descriptive error (no partial boot).
- Environment is modeled explicitly (`local` / `staging` / `production`); the placeholder
  secret is only tolerated in `local`.

## Files / code
- `src/config.rs`
  - `Config` — the config struct.
  - `Config::from_env()` — reads env, constructs, validates.
  - `Config::validate()` — fail-fast rules.
  - `Environment` — enum + `parse`/`is_local`.
  - `ConfigError::DefaultSecret` — error when a secret is still `changethis`.

## Contract
Environment variables:

| Var | Default | Required in prod | Notes |
|-----|---------|------------------|-------|
| `ENVIRONMENT` | `local` | no | `local`/`staging`/`production` |
| `BIND_ADDR` | `0.0.0.0:8080` | no | listen address |
| `SECRET_KEY` | `changethis` | **yes** | startup fails if unset/default outside `local` |
| `CORS_ORIGINS` | _(empty)_ | recommended | comma-separated origin allowlist |

Behavior:
- In `staging`/`production`, `SECRET_KEY == "changethis"` → `ConfigError::DefaultSecret` → exit non-zero.
- An unrecognized `ENVIRONMENT` value → `ConfigError::UnknownEnvironment` → exit non-zero
  (**fail-closed**: a typo like `prod` must never be silently treated as `local`).

## Status
✅ Implemented (placeholder check for `SECRET_KEY`). Extend the same fail-fast pattern to
future secrets (`POSTGRES_PASSWORD`, Telegram/SMTP tokens) when those are added.

## Open questions
- Adopt a config crate (`figment`/`config`) for layered files + env once config grows? (IDEAS.md §5)
