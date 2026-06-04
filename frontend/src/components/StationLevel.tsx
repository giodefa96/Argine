// Loads and renders one station's level + rain series: handles loading / error / empty / data.
// A period selector bounds the window (`from`) and sizes `limit` to the 10-minute ARPA
// cadence (~6 points/hour, with headroom), within the API's 10k cap. Long periods switch
// to server-side bucketing (level averaged, rain summed) — raw 10-minute data would not fit.

import { useState } from 'react'
import { useObservations } from '../hooks/queries'
import type { ObservationParams, Station } from '../lib/api'
import LevelChart from './LevelChart'

const PERIODS = [
  { key: '24h', label: '24 ore', hours: 24 },
  { key: '7d', label: '7 giorni', hours: 24 * 7 },
  { key: '30d', label: '30 giorni', hours: 24 * 30 },
  { key: '1y', label: '1 anno', hours: 24 * 365, bucket: '6h' },
  // The ingested archive starts 2021 (ARPA digitized history) — "everything" in day buckets.
  { key: 'all', label: 'Tutto (dal 2021)', hours: 24 * 365 * 6, bucket: '1d' },
] as const

type PeriodKey = (typeof PERIODS)[number]['key']

/** The selected period as a stable query window (computed outside render: `Date.now()`
 *  is impure, so it runs only in the state initializer and the change handler). */
function windowFor(key: PeriodKey) {
  const period = PERIODS.find((p) => p.key === key) ?? PERIODS[0]
  const bucket: ObservationParams['bucket'] = 'bucket' in period ? period.bucket : undefined
  // Raw 10-minute cadence ≈ 6 points/hour (with headroom); bucketed periods need one point
  // per bucket. Either way the API caps at 10k.
  const points =
    bucket === '1d' ? period.hours / 24 : bucket === '6h' ? period.hours / 6 : period.hours * 7
  return {
    key: period.key,
    from: new Date(Date.now() - period.hours * 3_600_000).toISOString(),
    limit: Math.min(Math.ceil(points * 1.1), 10_000),
    bucket,
  }
}

export default function StationLevel({ station }: { station: Station }) {
  const [{ key: periodKey, from, limit, bucket }, setWindow] = useState(() => windowFor('24h'))

  const level = useObservations(station.id, { from, limit, bucket, metric: 'level_m' })
  const rain = useObservations(station.id, { from, limit, bucket, metric: 'rain_mm' })

  const selector = (
    <label htmlFor="period-select">
      Periodo:{' '}
      <select
        id="period-select"
        name="period"
        value={periodKey}
        onChange={(e) => setWindow(windowFor(e.target.value as PeriodKey))}
      >
        {PERIODS.map((p) => (
          <option key={p.key} value={p.key}>
            {p.label}
          </option>
        ))}
      </select>
    </label>
  )

  if (level.isLoading) return <p>Caricamento dati…</p>
  if (level.isError) return <p role="alert">Errore nel caricamento dei dati.</p>
  if (!level.data || level.data.length === 0) {
    return (
      <section>
        {selector}
        <p>Nessun dato disponibile per il periodo.</p>
      </section>
    )
  }

  const latest = level.data[level.data.length - 1]
  return (
    <section>
      {selector}
      <p>
        Ultimo livello: <strong>{latest.value.toFixed(2)} m</strong>{' '}
        <time dateTime={latest.ts}>({new Date(latest.ts).toLocaleString()})</time>
      </p>
      <LevelChart level={level.data} rain={rain.data ?? []} />
    </section>
  )
}
