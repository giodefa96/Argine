import { useState } from 'react'
import StationLevel from './components/StationLevel'
import { useStations } from './hooks/queries'

export default function App() {
  const { data: stations, isLoading, isError } = useStations()
  const [selectedId, setSelectedId] = useState<number | null>(null)

  const selected = stations?.find((s) => s.id === selectedId) ?? stations?.[0] ?? null

  return (
    <main>
      <h1>Argine</h1>
      <p>Monitoraggio del livello del Seveso.</p>

      {isLoading && <p>Caricamento stazioni…</p>}
      {isError && <p role="alert">Errore nel caricamento delle stazioni.</p>}

      {stations && stations.length > 0 && (
        <>
          <label htmlFor="station-select">
            Stazione:{' '}
            <select
              id="station-select"
              name="station"
              value={selected?.id ?? ''}
              onChange={(e) => setSelectedId(Number(e.target.value))}
            >
              {stations.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          {selected && <StationLevel station={selected} />}
        </>
      )}
    </main>
  )
}
