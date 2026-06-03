// Loads and renders one station's level + rain series: handles loading / error / empty / data.

import { useObservations } from '../hooks/queries'
import type { Station } from '../lib/api'
import LevelChart from './LevelChart'

export default function StationLevel({ station }: { station: Station }) {
  const level = useObservations(station.id, { limit: 2000, metric: 'level_m' })
  const rain = useObservations(station.id, { limit: 2000, metric: 'rain_mm' })

  if (level.isLoading) return <p>Caricamento dati…</p>
  if (level.isError) return <p role="alert">Errore nel caricamento dei dati.</p>
  if (!level.data || level.data.length === 0) return <p>Nessun dato disponibile.</p>

  const latest = level.data[level.data.length - 1]
  return (
    <section>
      <p>
        Ultimo livello: <strong>{latest.value.toFixed(2)} m</strong>{' '}
        <time dateTime={latest.ts}>({new Date(latest.ts).toLocaleString()})</time>
      </p>
      <LevelChart level={level.data} rain={rain.data ?? []} />
    </section>
  )
}
