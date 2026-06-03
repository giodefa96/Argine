// Typed client for the Argine backend read API. Base URL comes from VITE_API_URL
// (set per environment); defaults to the local backend for `pnpm dev`.

const API_URL = (import.meta.env.VITE_API_URL as string | undefined) ?? 'http://localhost:8080'

export interface Station {
  id: number
  source: string
  external_id: string
  name: string
  river: string
  kind: 'hydrometric' | 'rain'
  lat: number
  lon: number
  created_at: string
}

export interface Observation {
  station_id: number
  ts: string
  metric: 'level_m' | 'rain_mm'
  value: number
}

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(`${API_URL}${path}`)
  if (!res.ok) {
    throw new Error(`API ${res.status} for ${path}`)
  }
  return (await res.json()) as T
}

export function fetchStations(): Promise<Station[]> {
  return getJson<Station[]>('/stations')
}

export interface ObservationParams {
  from?: string
  to?: string
  limit?: number
  metric?: 'level_m' | 'rain_mm'
}

export function fetchObservations(
  stationId: number,
  params: ObservationParams = {},
): Promise<Observation[]> {
  const q = new URLSearchParams()
  if (params.from) q.set('from', params.from)
  if (params.to) q.set('to', params.to)
  if (params.limit != null) q.set('limit', String(params.limit))
  if (params.metric) q.set('metric', params.metric)
  const qs = q.toString()
  return getJson<Observation[]>(`/stations/${stationId}/observations${qs ? `?${qs}` : ''}`)
}
