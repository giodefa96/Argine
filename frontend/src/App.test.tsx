import { screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'
import App from './App'
import { renderWithClient } from './test/utils'

// App renders the chart subtree, which pulls in uPlot (canvas, unsupported in jsdom).
vi.mock('uplot', () => {
  class U {
    destroy() {}
  }
  ;(U as unknown as { paths: unknown }).paths = { bars: () => () => {} }
  return { default: U }
})

test('renders the Argine heading', () => {
  renderWithClient(<App />)
  expect(screen.getByRole('heading', { name: /argine/i })).toBeInTheDocument()
})

test('mentions the Seveso', () => {
  renderWithClient(<App />)
  expect(screen.getByRole('main')).toHaveTextContent(/seveso/i)
})

test('lists a station from the API', async () => {
  renderWithClient(<App />)
  expect(await screen.findByRole('option', { name: 'Milano Niguarda' })).toBeInTheDocument()
})
