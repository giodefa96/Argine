import { render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import App from './App'

test('renders the Argine heading', () => {
  render(<App />)
  expect(screen.getByRole('heading', { name: /argine/i })).toBeInTheDocument()
})

test('mentions the Seveso', () => {
  render(<App />)
  expect(screen.getByRole('main')).toHaveTextContent(/seveso/i)
})
