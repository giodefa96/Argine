-- Weather forecasts (IDEAS.md §7): hourly forecast precipitation per station point.
-- The basin is approximated by the station coordinates for the MVP (IDEAS.md §12 Q3);
-- a real basin entity can replace station_id later. Every forecast run is kept
-- (keyed by run_ts) so revisions can be backtested against what was known at the time.
CREATE TABLE weather_forecast (
    station_id bigint NOT NULL REFERENCES station (id) ON DELETE CASCADE,
    run_ts     timestamptz NOT NULL,  -- when this forecast was issued/fetched
    ts         timestamptz NOT NULL,  -- the hour the forecast is for
    rain_mm    double precision NOT NULL,
    model      text NOT NULL,         -- forecast model, e.g. 'best_match'
    UNIQUE (station_id, model, run_ts, ts)
);

-- Partition on ts like observation; the unique key above covers the partition column.
SELECT create_hypertable('weather_forecast', by_range('ts'));

-- Supports "latest run for a station" and "one run's series over a time range".
CREATE INDEX weather_forecast_station_run_idx
    ON weather_forecast (station_id, run_ts DESC, ts);
