import { screen } from '@testing-library/react'
import { http, HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { sampleStations } from '../test/handlers'
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
})
