-- Bootstrap migration: enable the TimescaleDB extension and a smoke table that proves
-- the migration + DB wiring works end to end. The domain schema (station, observation,
-- threshold) lands in the next feature.

CREATE EXTENSION IF NOT EXISTS timescaledb;

CREATE TABLE IF NOT EXISTS schema_smoke (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    checked_at timestamptz NOT NULL DEFAULT now()
);
