# CLAUDE.md

Guidance for AI agents working in this repository. Project context first, then the
general behavioral guidelines.

## Project: Argine

River level monitoring & short-term forecasting from meteorological models, with risk
alerts. **First target river: the Seveso** (Brianza/Como basin → Milan).

- **Status:** design phase. No application code yet — `backend/` and `frontend/` currently
  hold only config (`deny.toml`, `.npmrc`). Build/test commands will be added with the scaffold.
- **Design doc:** [`IDEAS.md`](./IDEAS.md) — architecture, stack rationale, forecast model
  (point + spatial), deploy, roadmap. Read it before proposing changes.

### Stack (decided)
- **Backend:** Rust — Axum, Tokio, `sqlx` + PostgreSQL/**TimescaleDB**, `reqwest`,
  `tracing`. ML inference via **ONNX** (`ort`/`tract`) inside Rust.
- **Frontend:** React + TypeScript (Vite), TanStack Query, MapLibre GL, uPlot, Tailwind. PWA.
- **Python (offline only):** ML training (XGBoost/LightGBM → ONNX) and geo/HAND preprocessing
  (GDAL/pysheds/WhiteboxTools). Never on the runtime path.
- **Deploy:** everything in Docker, orchestrated with Docker Compose.

### Planned layout
```
backend/        Rust service (Axum)            frontend/   React PWA
.github/workflows/  CI security gate           IDEAS.md    design doc
SECURITY.md     security baseline + checklist   Makefile    dev entry points
```

### Conventions
- Documentation and code comments in **English**.
- Data sources: ARPA Lombardia (hydrometry), Open-Meteo (rain forecast), PGRA/PAI + DTM (geo).
- **Docs are mandatory and live next to the code.** `backend/docs/` and `frontend/docs/` are
  the low-level source of truth: each holds an `ARCHITECTURE.md` plus a `features/<name>.md`
  per feature (template in each `docs/README.md`). **Every PR that adds or changes a feature
  must add/update its `features/` doc and `ARCHITECTURE.md` in the same PR** — docs drift is
  treated as a bug. From `docs/` alone one should understand the whole project.

## Security is non-negotiable — read [`SECURITY.md`](./SECURITY.md)

This project is security-by-design. When writing or reviewing code, these are hard rules:
- **Secrets only via env**; commit only `.env.example` (placeholders `changethis`). Backend
  must **fail-fast at startup** if a secret is still the default in non-local environments.
- **Never log secrets, tokens, or passwords.** Never echo raw DB errors to clients.
- **SQL:** `sqlx` parameterized queries only — never string-format SQL.
- **Input:** validate all request bodies (`serde` + `validator`); bound coordinates, ranges, pagination.
- **Auth:** JWT (`jsonwebtoken`, `exp` validated) + Argon2id passwords (`argon2`). Public read
  endpoints stay open; gate only writes, admin, and subscriber data.
- **CORS:** explicit allowlist, never `Any` in production.
- **Supply chain:** install/build with the lockfile (`cargo build --locked`; frontend uses
  **pnpm** — `pnpm install --frozen-lockfile`, never `npm install`). pnpm blocks install
  scripts by default; allowlist in `frontend/pnpm-workspace.yaml`. New deps must pass
  `cargo-deny`/dependency-review. Assume dependency code runs at build time; prefer
  `cargo-vet` + sandboxed builds.
- **Keep secrets OUT of the build shell environment** — build scripts inherit env vars.

## Testing & quality (mandatory per feature)
Every feature ships with tests, as exhaustive as the risk warrants (don't chase coverage %
on trivial code; test the risky logic hard: forecast model, alert thresholds, flood mapping,
auth, config/security paths).
- **Backend:** `cargo test` — unit (`#[cfg(test)]`) + integration (`tests/`, router via
  `oneshot`). External APIs (ARPA/Open-Meteo) must be mocked (`wiremock`), never called in CI.
- **Frontend:** **Vitest** + Testing Library for components; **Playwright** for E2E on
  **critical journeys only**. Mock the backend (MSW) — no real API calls in tests.
- **Coverage:** no-drop ratchet (Codecov `project: auto`); coverage may hold or rise, not fall.
- **Lint/format:** Rust `fmt`+`clippy -D warnings`; frontend ESLint + Prettier; Python
  (geo/ML) `ruff` + `mypy`. All enforced by pre-commit and CI.
- **CI is staged:** fast lint+unit first, then E2E (see `.github/workflows/test.yml`).

## Definition of Done (per feature)
Code **+** tests **+** `docs/features/<name>.md` and `ARCHITECTURE.md` updated **+** all gates
green (security + lint + tests). A PR missing any of these is not done.

## Developer commands
- `make hooks` — install local git hooks (pre-commit + pre-push: secrets, lint, format, tests).
- `make security` — local security gate (`cargo-deny`, `pnpm audit`, gitleaks).
- `make lint` — fmt/clippy + eslint/prettier checks.
- `make test` — backend + frontend unit/integration tests. `make e2e` — Playwright.
- CI workflows in `.github/workflows/` run on every PR: `security.yml`, `test.yml`,
  `dependency-review.yml`, `codeql.yml`, `claude-review.yml`.

---

# General behavioral guidelines

Behavioral guidelines to reduce common LLM coding mistakes.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
