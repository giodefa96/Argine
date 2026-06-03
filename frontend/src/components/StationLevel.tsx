// Loads and renders one station's level series: handles loading / error / empty / data.

import { useObservations } from '../hooks/queries'
import type { Station } from '../lib/api'
import LevelChart from './LevelChart'

export default function StationLevel({ station }: { station: Station }) {
  const { data, isLoading, isError } = useObservations(station.id, { limit: 2000 })

  if (isLoading) return <p>Caricamento dati…</p>
  if (isError) return <p role="alert">Errore nel caricamento dei dati.</p>
  if (!data || data.length === 0) return <p>Nessun dato disponibile.</p>

  const latest = data[data.length - 1]
  return (
    <section>
      <p>
        Ultimo livello: <strong>{latest.value.toFixed(2)} m</strong>{' '}
        <time dateTime={latest.ts}>({new Date(latest.ts).toLocaleString()})</time>
      </p>
      <LevelChart observations={data} />
    </section>
  )
}
