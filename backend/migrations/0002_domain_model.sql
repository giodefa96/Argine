-- Domain model: the core entities the rest of the system builds on — measurement
-- stations, their alert thresholds, and the observation time series.
-- Builds on 0001 (TimescaleDB extension already enabled).

-- Measurement stations (hydrometric level gauges or rain gauges). One row per physical
-- station, identified within its data provider by (source, external_id).
CREATE TABLE station (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source      text NOT NULL,                       -- data provider, e.g. 'arpa_lombardia'
    external_id text NOT NULL,                       -- station id within that provider
    name        text NOT NULL,
    river       text NOT NULL,
    kind        text NOT NULL CHECK (kind IN ('hydrometric', 'rain')),
    lat         double precision NOT NULL CHECK (lat BETWEEN -90 AND 90),
    lon         double precision NOT NULL CHECK (lon BETWEEN -180 AND 180),
    created_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source, external_id)
);

-- Civil-Protection alert thresholds: at most one row per (station, severity level).
CREATE TABLE threshold (
    station_id bigint NOT NULL REFERENCES station (id) ON DELETE CASCADE,
    level      text NOT NULL CHECK (level IN ('yellow', 'orange', 'red')),
    value_m    double precision NOT NULL,
    PRIMARY KEY (station_id, level)
);

-- Observations: time series of measured values (level in metres, rain in mm).
-- No surrogate key — the natural key (station_id, metric, ts) makes re-ingestion
-- idempotent (ON CONFLICT DO UPDATE) and includes the hypertable partition column.
CREATE TABLE observation (
    station_id bigint NOT NULL REFERENCES station (id) ON DELETE CASCADE,
    ts         timestamptz NOT NULL,
    metric     text NOT NULL CHECK (metric IN ('level_m', 'rain_mm')),
    value      double precision NOT NULL,
    UNIQUE (station_id, metric, ts)
);

-- Partition observation on ts (TimescaleDB hypertable). The unique key above already
-- includes ts, satisfying the "unique index must cover the partition column" rule.
SELECT create_hypertable('observation', by_range('ts'));

-- Supports the planned read query: one station's series over a time range.
CREATE INDEX observation_station_ts_idx ON observation (station_id, ts DESC);
