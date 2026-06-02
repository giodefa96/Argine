# Frontend architecture

React + TypeScript single-page app built with **Vite**, managed with **pnpm**. This document
tracks the *implemented* structure; planned pieces are marked as such.

## Tooling
- **Build:** Vite 8 (`vite.config.ts`, `@vitejs/plugin-react`).
- **Language:** TypeScript 6, strict mode (`tsconfig.json`).
- **Package manager:** **pnpm** — install scripts blocked by default; allowlist in
  `pnpm-workspace.yaml` (`onlyBuiltDependencies: [esbuild]`). Lockfile `pnpm-lock.yaml` committed.
- **Scripts:** `dev` (vite), `build` (`tsc --noEmit && vite build`), `preview`.

## Entry flow
```
index.html ──▶ src/main.tsx ──▶ <App/>
   (#root)        createRoot + StrictMode
```

## Structure (current)
| File | Responsibility |
|------|----------------|
| `index.html` | HTML host, mounts `#root`, loads `src/main.tsx` |
| `src/main.tsx` | React root, `StrictMode` |
| `src/App.tsx` | top-level component (placeholder landing) |
| `src/vite-env.d.ts` | Vite client type refs |

## Planned components (not yet implemented)
Tracked in [`IDEAS.md`](../../IDEAS.md); each gets a `features/` doc when built:
- Data layer: **TanStack Query** against the backend REST API.
- Charts: **uPlot** (river level / forecast time series).
- Map: **MapLibre GL** (stations + dynamic risk zones + PGRA/PAI layers).
- **PWA** + Web Push for alerts.
- Routing, styling (**Tailwind**).
