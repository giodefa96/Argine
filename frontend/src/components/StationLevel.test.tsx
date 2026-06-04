import { fireEvent, screen, waitFor } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { sampleObservations, sampleStations } from '../test/handlers'
import { server } from '../test/server'
import { renderWithClient } from '../test/utils'
import StationLevel from './StationLevel'

// uPlot draws to a canvas, which jsdom doesn't implement — mock it with a no-op class
// (constructed via `new`) plus the static paths.bars builder LevelChart uses for rain.
// Real rendering is covered by the E2E test.
vi.mock('uplot', () => {
  class U {
    destroy() {}
  }
  ;(U as unknown as { paths: unknown }).paths = { bars: () => () => {} }
  return { default: U }
})

const station = sampleStations[0]

describe('StationLevel', () => {
  it('renders the latest value and the chart on success', async () => {
    renderWithClient(<StationLevel station={station} />)

    expect(await screen.findByText(/Ultimo livello/)).toBeInTheDocument()
    expect(screen.getByText(/0\.70 m/)).toBeInTheDocument()
    expect(screen.getByTestId('level-chart')).toBeInTheDocument()
  })

  it('shows the empty state when there are no observations', async () => {
    server.use(
      http.get('http://localhost:8080/stations/:id/observations', () => HttpResponse.json([])),
    )
    renderWithClient(<StationLevel station={station} />)
    expect(await screen.findByText(/Nessun dato/)).toBeInTheDocument()
  })

  it('shows the error state on failure', async () => {
    server.use(
      http.get(
        'http://localhost:8080/stations/:id/observations',
        () => new HttpResponse(null, { status: 500 }),
      ),
    )
    renderWithClient(<StationLevel station={station} />)
    expect(await screen.findByRole('alert')).toHaveTextContent(/Errore/)
  })

  it('changing the period re-queries with a wider window', async () => {
    const froms: string[] = []
    server.use(
      http.get('http://localhost:8080/stations/:id/observations', ({ request }) => {
        const url = new URL(request.url)
        froms.push(url.searchParams.get('from') ?? '')
        return HttpResponse.json(sampleObservations)
      }),
    )
    renderWithClient(<StationLevel station={station} />)
    await screen.findByText(/Ultimo livello/)

    fireEvent.change(screen.getByRole('combobox', { name: /periodo/i }), {
      target: { value: '7d' },
    })
    await screen.findByText(/Ultimo livello/)
    await waitFor(() => expect(froms.length).toBeGreaterThan(2))

    const dayMs = 24 * 3_600_000
    const first = Date.now() - new Date(froms[0]).getTime()
    const widened = Date.now() - new Date(froms[froms.length - 1]).getTime()
    expect(first).toBeLessThan(2 * dayMs) // 24h window
    expect(widened).toBeGreaterThan(6 * dayMs) // 7-day window
  })
})
