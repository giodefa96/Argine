// Default MSW handlers for the backend read API used by component/data tests.
import { http, HttpResponse } from 'msw'
import type { Observation, Station } from '../lib/api'

const API = 'http://localhost:8080'

export const sampleStations: Station[] = [
  {
    id: 1,
    source: 'arpa_lombardia',
    external_id: '3118',
    name: 'Milano Niguarda',
    river: 'Seveso',
    kind: 'hydrometric',
    lat: 45.5258,
    lon: 9.1922,
    created_at: '2026-06-03T00:00:00Z',
  },
]

export const sampleObservations: Observation[] = [
  { station_id: 1, ts: '2026-06-02T10:00:00Z', metric: 'level_m', value: 0.5 },
  { station_id: 1, ts: '2026-06-02T11:00:00Z', metric: 'level_m', value: 0.7 },
]

export const handlers = [
  http.get(`${API}/stations`, () => HttpResponse.json(sampleStations)),
  http.get(`${API}/stations/:id/observations`, () => HttpResponse.json(sampleObservations)),
]
