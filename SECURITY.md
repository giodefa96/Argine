# Argine — Security by Design

> Security baseline for the project, adapted from the practices of the
> [Full-Stack FastAPI Template](https://github.com/fastapi/full-stack-fastapi-template)
> and translated to our **Rust + React + Docker** stack. The goal is to bake security
> in from day one rather than retrofit it later.

## Context note

Most of Argine's data (river levels, forecasts, risk maps) is **public, read-only**.
Authentication therefore mostly protects: **admin operations** (managing stations,
thresholds, retraining), **alert subscriptions** (who gets notified), and **write
endpoints**. Keep this asymmetry in mind — don't gate public read endpoints behind auth,
but lock down everything that mutates state or exposes personal data (subscriber contacts).

---

## 1. Authentication — JWT

**Template:** JWT access tokens, HS256 (HMAC-SHA256) via PyJWT, payload with `sub` + `exp`.

**Argine (Rust):**
- Crate: [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken).
- Algorithm: **HS256** with a symmetric `SECRET_KEY` (single backend, symmetric is fine).
  If we ever split services or need third-party verification, switch to **RS256/EdDSA**
  (asymmetric).
- Claims: `sub` (user/subject id), `exp` (expiry), `iat`, and `iss`/`aud` if useful.
- **Validate `exp` on every request** — `jsonwebtoken` does this automatically when you
  set `Validation`. Reject expired/invalid tokens with `401`.
- Extract & validate the token in an Axum **extractor / middleware**, not per-handler.

```rust
// sketch
let token = encode(&Header::new(Algorithm::HS256), &claims, &encoding_key)?;
let data  = decode::<Claims>(&token, &decoding_key, &Validation::new(Algorithm::HS256))?;
```

## 2. Password hashing — Argon2

**Template:** `pwdlib` with **Argon2** (primary) + **bcrypt** (legacy), auto-upgrading old
hashes on login via `verify_and_update`.

**Argine (Rust):**
- Crate: [`argon2`](https://crates.io/crates/argon2) (Argon2id). This is the modern,
  memory-hard default — prefer it over bcrypt for new projects.
- Use the `password-hash` traits with a random salt (`SaltString::generate`).
- **Never** store or log plaintext passwords. Store only the PHC-format hash string.
- If we later need to migrate hash parameters, replicate the "rehash on successful login"
  pattern (check params, re-hash if outdated).

```rust
// sketch
let salt = SaltString::generate(&mut OsRng);
let hash = Argon2::default().hash_password(pw.as_bytes(), &salt)?.to_string();
// verify
Argon2::default().verify_password(pw.as_bytes(), &PasswordHash::new(&hash)?).is_ok();
```

## 3. Secrets management

**Template:** `SECRET_KEY` defaults to `secrets.token_urlsafe(32)`; placeholder values are
literally `"changethis"`; a validator **raises an error in prod/staging** (warning in local)
if a secret is still the default. Secrets passed as env vars, never committed.

**Argine:**
- All secrets via **environment variables** (`.env` for local, real env/secret store in prod).
  Never commit `.env`; commit only `.env.example` with placeholder `changethis` values.
- **Fail-fast validation at startup**: on boot, the backend checks that `SECRET_KEY`,
  `POSTGRES_PASSWORD`, `FIRST_SUPERUSER_PASSWORD`, Telegram/SMTP tokens, etc. are **not**
  the placeholder. In non-local environments → **panic/exit**; in local → log a warning.
- Generate strong keys: `openssl rand -base64 32` (or `head -c32 /dev/urandom | base64`).
- Config via `figment`/`config` crate; validate the parsed struct before serving traffic.
- Add `.env`, `*.key`, `*.pem` to `.gitignore`.

```
# .env.example
SECRET_KEY=changethis
POSTGRES_PASSWORD=changethis
FIRST_SUPERUSER=admin@example.com
FIRST_SUPERUSER_PASSWORD=changethis
ENVIRONMENT=local   # local | staging | production
```

## 4. CORS

**Template:** explicit `BACKEND_CORS_ORIGINS` allowlist, combined with the frontend host.

**Argine (Rust):**
- Crate: [`tower-http`](https://crates.io/crates/tower-http) `CorsLayer`.
- **Explicit origin allowlist** from config — **never** `allow_origin(Any)` in production.
- Restrict methods/headers to what's actually used; allow credentials only if needed.

```rust
let cors = CorsLayer::new()
    .allow_origin(allowed_origins)        // from config, exact list
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE]);
```

## 5. Token expiry & sessions

**Template:** `ACCESS_TOKEN_EXPIRE_MINUTES = 60*24*8` (8 days).

**Argine:**
- 8 days is long. For admin/write tokens prefer **short-lived access tokens** (e.g. 15–60 min)
  + a **refresh token** flow, or at least a shorter expiry.
- Keep `exp` mandatory and validated. Consider token revocation (denylist by `jti`) for
  logout/compromise if we add stateful sessions.

## 6. Transport security — HTTPS/TLS

**Template:** automatic HTTPS via **Traefik** reverse proxy.

**Argine:**
- TLS terminated at a reverse proxy in front of the backend — **Caddy** (automatic Let's
  Encrypt, zero-config) or **Traefik**. Already the plan in `IDEAS.md` §10-bis.
- Redirect HTTP→HTTPS. Set **HSTS**, and consider security headers
  (`X-Content-Type-Options: nosniff`, `Referrer-Policy`, a sensible `Content-Security-Policy`
  for the React app) via the proxy or `tower-http` `SetResponseHeaderLayer`.

## 7. Input validation & injection protection

**Template:** Pydantic / SQLModel validate input and parameterize queries.

**Argine:**
- **SQL**: use `sqlx` with **parameterized queries only** — never string-format SQL.
  `sqlx`'s compile-time `query!`/`query_as!` macros make this the natural path.
- **Input**: validate/deserialize request bodies with `serde` + a validation crate
  (e.g. [`validator`](https://crates.io/crates/validator)) — bounds-check coordinates,
  date ranges, pagination limits, etc.
- **Output**: never echo raw DB errors to clients; map to generic messages + log details.

## 8. Admin / superuser bootstrap

**Template:** first superuser created from env vars.

**Argine:** bootstrap the initial admin from `FIRST_SUPERUSER` / `FIRST_SUPERUSER_PASSWORD`
on first migration/run, hashing the password with Argon2. Enforce non-default password
(see §3). Use a **role/permission** field on users; gate admin endpoints on it.

## 9. Email / notifications

**Template:** email password recovery; Mailcatcher for local testing.

**Argine:**
- Password recovery tokens must be **single-use, short-lived, and random** (don't reuse the
  auth `SECRET_KEY` payload as a reset link).
- For local dev, use a catch-all SMTP (Mailpit/Mailcatcher) so test emails never leave the box.
- Telegram bot token & SMTP creds are secrets (§3). Validate subscriber input; rate-limit
  subscription endpoints to prevent abuse.

## 10. Container & deployment hardening

**Template:** Docker isolation, CI/CD automation.

**Argine:**
- **Multi-stage builds**; run the backend on a minimal runtime (`debian-slim`/`distroless`)
  as a **non-root user**.
- No secrets baked into images — inject at runtime via env/secret store.
- Pin base image digests; keep dependencies updated (`cargo audit` / Dependabot,
  `npm audit` for the frontend).
- Persistent DB volumes; don't expose Postgres to the public network (only on the internal
  Docker network).
- Principle of least privilege for the DB user the app connects with.

## 11. CI security pipeline

Enforced automatically via GitHub Actions in [`.github/workflows/`](./.github/workflows/).
These run on **every pull request** (and push to `main`) and are intended to be **required
status checks** in branch protection, so nothing merges without passing them.

| Workflow | What it checks |
|---|---|
| `security.yml` | **Backend**: `cargo-deny` (RustSec advisories + license allowlist + banned crates + trusted sources, configured in [`backend/deny.toml`](./backend/deny.toml)). **Frontend**: `pnpm audit` (fail on high/critical). **Repo**: `gitleaks` secret scanning. |
| `dependency-review.yml` | On PRs only: vets **newly introduced** dependencies, blocking vulnerable ones or disallowed licenses **before they are merged/installed**. |
| `codeql.yml` | SAST for Rust and JS/TS (PR + push + weekly schedule). |
| `test.yml` | Staged quality/correctness: lint + unit/integration tests, then E2E. Not a security workflow but runs the same gate (coverage no-drop via Codecov). |
| `claude-review.yml` | AI agent review of the PR diff (advisory; auth via Claude Pro/Max OAuth token in secret `CLAUDE_CODE_OAUTH_TOKEN`, generated with `claude setup-token`). |

### Supply-chain vulnerability databases consulted
Dependencies are cross-referenced against live, online advisory databases at multiple points:

| Check | Database | When |
|---|---|---|
| `cargo-deny` (backend) | **RustSec Advisory Database** | every PR + push + local `make security` |
| `pnpm audit` (frontend) | **GitHub Advisory Database** (npm advisories) | every PR + local |
| `dependency-review` | **GitHub Advisory Database** (cargo + npm) | every PR, on the diff |
| **Dependabot alerts** | **GitHub Advisory Database** | continuously, even after merge |

So a vulnerable dependency is caught (a) on the PR that introduces it, and (b) continuously
afterwards if a *new* advisory is later published against an already-merged version.
CodeQL is **not** a dependency scanner — it's SAST on our own source. gitleaks/GitGuardian
scan for leaked secrets, not vulnerabilities.

Notes:
- Workflows **self-skip** per area until `backend/` and `frontend/` exist.
- `dependency-review` requires the repo's Dependency Graph (enabled).
- Required checks on protected branches: `secret-scan` + `dependency-review` (enforced on
  `main` and `develop`).
- **Dependabot** alerts + automated security fixes: enabled.

## 12. Local developer defense

The CI gate is the *last* line. The *first* line runs on your machine, because once a
secret is committed and pushed it must be considered leaked (rotate it) even if CI catches it.

### Git hooks (mirror CI locally)
- Managed with the [`pre-commit`](https://pre-commit.com/) framework — config in
  [`.pre-commit-config.yaml`](./.pre-commit-config.yaml). Install with `make hooks`.
- **pre-commit stage** (fast, every commit): `gitleaks` secret scan, private-key detection,
  hygiene checks, `cargo fmt --check`, frontend ESLint + Prettier, Python `ruff` (lint+format,
  inert until `.py` files exist).
- **pre-push stage** (heavier, before push): `cargo clippy -D warnings`, `cargo test`,
  `cargo deny check`, frontend `tsc --noEmit` + `vitest` + `pnpm audit`, Python `mypy` —
  the same checks CI runs, so failures are caught before the PR exists.
- Run on demand: `make security` (security gate), `make lint`, `make test`.

## 13. Supply-chain defense at install/build time

> The hard truth: `cargo build`/`cargo run` and `npm install` **execute dependency code on
> your machine** — Rust `build.rs` build scripts and proc-macros run at compile time (by
> design, and there is **no flag to disable them** in a normal build); npm lifecycle scripts
> run at install time. This happens **before** `cargo-deny`/`npm audit` ever run. There is no
> silver bullet — the defense is layered, and built on one assumption: *dependency code may
> run, so make sure only vetted code runs, and that when it runs it has nothing to steal and
> nowhere to send it.*

**Layer 1 — Determinism: run only the code you reviewed.**
- Commit lockfiles (`Cargo.lock`, `package-lock.json`) — they pin exact versions **with
  integrity hashes**, so you get the same bytes every time.
- Build/install with the locked set, never resolving fresh: `cargo build --locked`
  (`--frozen` for fully offline), and **`npm ci`** (not `npm install`).
- Pin exact versions, no caret ranges (`frontend/.npmrc` sets `save-exact=true`).

**Layer 2 — Block / allowlist install scripts.**
- npm: the cleanest option is **pnpm**, which blocks install scripts by default and lets you
  allowlist only the packages that may run them (`onlyBuiltDependencies`). With npm,
  `ignore-scripts=true` is the equivalent but can break native-build packages (e.g. esbuild) —
  see [`frontend/.npmrc`](./frontend/.npmrc) for the trade-off.
- Rust build scripts can't be disabled → handled by Layer 3 + 4 instead.

**Layer 3 — Vet crates before their code runs (Rust's real answer).**
- Adopt [`cargo-vet`](https://mozilla.github.io/cargo-vet/): a CI + local gate that only
  allows crate **versions that have been audited** (by you or by trusted parties whose audits
  you import). New/unvetted versions fail the check, so unreviewed `build.rs` never reaches a
  normal build path. (`cargo-crev` is the distributed-review alternative.)
- Keep the dependency tree small; review new deps in PRs (dependency-review action already
  flags them).

**Layer 4 — Sandbox the build (the catch-all for "code will run anyway").**
- Do dependency installs and builds inside a **container / devcontainer** that has
  **no real secrets** and **restricted network egress**. Then even a malicious build script
  executes in a box with nothing valuable to exfiltrate and no outbound path.
- **Keep secrets OUT of your interactive shell environment.** A build script inherits your
  env vars — if `AWS_*`, tokens, or `SECRET_KEY` live in your shell, they're readable.
  Load secrets only into the app at runtime (`.env` read by the app, or a secrets manager),
  not `export`ed into the shell you build in.
- CI already runs in a fresh, isolated, secret-minimal runner — prefer it for first builds of
  new/updated dependencies.

**Action items (once code lands):**
- [ ] Set up `cargo-vet` + initial audit set; add `cargo vet` to CI and `make security`.
- [ ] Decide npm vs **pnpm** for the frontend (pnpm recommended for script allowlisting).
- [ ] Add a `devcontainer` / build container with no secrets + egress limits.

## 14. Operational practices

- Structured logging with `tracing` — **never log secrets, tokens, or passwords**.
- Rate limiting on auth and subscription endpoints (`tower-governor` or proxy-level).
- A real disclosure policy (how to report vulnerabilities) before going public.

---

## Quick checklist (security-by-design baseline)

- [ ] JWT auth (`jsonwebtoken`, HS256), `exp` always validated, short-lived admin tokens.
- [ ] Argon2id password hashing (`argon2`), never plaintext.
- [ ] All secrets via env; startup **fail-fast** if any secret is still `changethis` in prod.
- [ ] `.env` gitignored; only `.env.example` committed.
- [ ] CORS explicit allowlist (no `Any` in prod).
- [ ] TLS at Caddy/Traefik + HSTS + security headers; HTTP→HTTPS redirect.
- [ ] `sqlx` parameterized queries only; `serde` + `validator` on all inputs.
- [ ] Admin gated by role; non-default bootstrap password enforced.
- [ ] Single-use, short-lived password-reset tokens.
- [ ] Non-root container, distroless/slim, no secrets in images, internal-only DB.
- [ ] CI security gate (`cargo-deny`, `pnpm audit`, dependency-review, CodeQL, gitleaks) — required checks on `main`.
- [ ] Local git hooks installed (`make hooks`) — secret scan + checks before commit/push.
- [ ] Lockfiles committed; build/install locked (`cargo build --locked`, `pnpm install --frozen-lockfile`).
- [x] Install-script policy: **pnpm** (scripts blocked by default; allowlist in `frontend/pnpm-workspace.yaml`).
- [ ] `cargo-vet` gating unvetted crate versions; small dependency tree.
- [ ] Builds of untrusted deps run sandboxed; **secrets kept out of the build shell env**.
- [ ] No secrets in logs; rate limiting on auth.
