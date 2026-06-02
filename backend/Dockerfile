# Multi-stage build → minimal, non-root runtime image (SECURITY.md §10).

# --- Build stage -------------------------------------------------------------
# Pinned by digest for reproducible builds (SECURITY.md §10); kept current by Dependabot.
FROM rust:1-slim@sha256:26abcef3d79b8d890c4ceb17093154573e1f6479cf6dd7c1450043b8458350f6 AS builder
WORKDIR /app

# Cache dependencies separately from source for faster rebuilds.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
# migrations/ is embedded at compile time by sqlx::migrate!(), so it must be present here.
COPY migrations ./migrations
# Touch so cargo rebuilds with the real sources.
RUN touch src/main.rs && cargo build --release --locked

# --- Runtime stage -----------------------------------------------------------
# distroless: no shell, no package manager, runs as a non-root user.
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:bd2899c12b335c827750ccf2359879eab09c09b206023dcebea408947d54127c
WORKDIR /app
COPY --from=builder /app/target/release/argine-backend /usr/local/bin/argine-backend
USER nonroot
EXPOSE 8080
ENTRYPOINT ["argine-backend"]
