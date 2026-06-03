import { http, HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { server } from '../test/server'
import { fetchObservations, fetchStations } from './api'

describe('api client', () => {
  it('fetchStations returns the station list', async () => {
    const stations = await fetchStations()
    expect(stations).toHaveLength(1)
    expect(stations[0].name).toBe('Milano Niguarda')
  })

  it('fetchObservations forwards query params and returns data', async () => {
    let url: URL | undefined
    server.use(
      http.get('http://localhost:8080/stations/:id/observations', ({ request, params }) => {
        url = new URL(request.url)
        expect(params.id).toBe('1')
        return HttpResponse.json([])
      }),
    )

    const obs = await fetchObservations(1, {
      from: '2026-06-01T00:00:00Z',
      limit: 500,
    })

    expect(obs).toEqual([])
    expect(url?.searchParams.get('from')).toBe('2026-06-01T00:00:00Z')
    expect(url?.searchParams.get('limit')).toBe('500')
  })

  it('throws on a non-2xx response', async () => {
    server.use(
      http.get('http://localhost:8080/stations', () => new HttpResponse(null, { status: 500 })),
    )
    await expect(fetchStations()).rejects.toThrow(/API 500/)
  })
})
