// Thin React wrapper around uPlot for the level time-series. uPlot is a fast vanilla-JS
// canvas charting lib, so we drive it imperatively from an effect and rebuild on data change.

import { useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import type { Observation } from '../lib/api'

export default function LevelChart({ observations }: { observations: Observation[] }) {
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    const xs = observations.map((o) => Math.round(new Date(o.ts).getTime() / 1000))
    const ys = observations.map((o) => o.value)
    const data: uPlot.AlignedData = [xs, ys]

    const opts: uPlot.Options = {
      width: el.clientWidth || 640,
      height: 320,
      scales: { x: { time: true } },
      series: [
        {},
        {
          label: 'Livello (m)',
          stroke: '#2563eb',
          width: 2,
          points: { show: observations.length < 60 },
        },
      ],
      axes: [{}, { label: 'm' }],
    }

    const chart = new uPlot(opts, data, el)
    return () => chart.destroy()
  }, [observations])

  return <div ref={containerRef} data-testid="level-chart" />
}
