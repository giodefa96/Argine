# Feature: App shell

## Overview
The minimal application shell: the HTML host, the React root, and a top-level `App`
component that currently renders a placeholder landing view. It's the mount point every
future feature plugs into.

## Design
Standard Vite React entry: `index.html` provides a `#root` div and loads the ES module
`src/main.tsx`, which creates the React root inside `StrictMode` and renders `<App/>`.

## Files / code
- `index.html` — `#root`, script `src/main.tsx`
- `src/main.tsx` — `createRoot(...).render(<StrictMode><App/></StrictMode>)`
- `src/App.tsx` — `App()` placeholder content

## Contract
- No props, no routes, no API calls yet.
- Renders a heading and a one-line description.

## Status
✅ Implemented (placeholder). Will gain routing, a layout, and the data provider
(TanStack Query) as features land.

## Open questions
- Routing library and app layout shape (sidebar + map + charts?) — to define with the first
  data-backed view.
