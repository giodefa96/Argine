# Multi-stage build → minimal, non-root runtime image (SECURITY.md §10).

# --- Build stage -------------------------------------------------------------
FROM rust:1-slim AS builder
WORKDIR /app

# Cache dependencies separately from source for faster rebuilds.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
# Touch so cargo rebuilds with the real sources.
RUN touch src/main.rs && cargo build --release --locked

# --- Runtime stage -----------------------------------------------------------
# distroless: no shell, no package manager, runs as a non-root user.
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /app/target/release/argine-backend /usr/local/bin/argine-backend
USER nonroot
EXPOSE 8080
ENTRYPOINT ["argine-backend"]
