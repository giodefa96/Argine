// Thin React wrapper around uPlot for the level (+ optional rain) time-series. uPlot is a
// fast vanilla-JS canvas lib, so we drive it imperatively from an effect and rebuild on data
// change. Level is a line on the left y axis; rain is bars on a right y axis (so the two
// correlate visually — rain upstream precedes the level rise).

import { useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import type { Observation } from '../lib/api'

export default function LevelChart({
  level,
  rain = [],
}: {
  level: Observation[]
  rain?: Observation[]
}) {
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    // Align both series on the union of their timestamps so uPlot shares one x axis.
    const rainByTs = new Map(
      rain.map((o) => [Math.round(new Date(o.ts).getTime() / 1000), o.value]),
    )
    const levelByTs = new Map(
      level.map((o) => [Math.round(new Date(o.ts).getTime() / 1000), o.value]),
    )
    const xs = [...new Set([...levelByTs.keys(), ...rainByTs.keys()])].sort((a, b) => a - b)
    const levelYs = xs.map((t) => levelByTs.get(t) ?? null)
    const rainYs = xs.map((t) => rainByTs.get(t) ?? null)

    const hasRain = rain.length > 0
    const data: uPlot.AlignedData = hasRain ? [xs, levelYs, rainYs] : [xs, levelYs]

    const series: uPlot.Series[] = [
      {},
      {
        label: 'Livello (m)',
        scale: 'm',
        stroke: '#2563eb',
        width: 2,
        points: { show: level.length < 60 },
      },
    ]
    if (hasRain) {
      // uPlot's bar-path builder lives under a loosely-typed namespace; alias to avoid `any`.
      const barsPath = (
        uPlot as unknown as { paths: { bars: (opts: object) => unknown } }
      ).paths.bars({ size: [0.6, 100] })
      series.push({
        label: 'Pioggia (mm)',
        scale: 'mm',
        stroke: '#0891b2',
        fill: 'rgba(8,145,178,0.35)',
        paths: barsPath as uPlot.Series.PathBuilder,
        points: { show: false },
      })
    }

    const opts: uPlot.Options = {
      width: el.clientWidth || 640,
      height: 320,
      scales: { x: { time: true } },
      series,
      axes: [
        {},
        { scale: 'm', label: 'm' },
        ...(hasRain ? [{ scale: 'mm', label: 'mm', side: 1, grid: { show: false } }] : []),
      ],
    }

    const chart = new uPlot(opts, data, el)
    return () => chart.destroy()
  }, [level, rain])

  return <div ref={containerRef} data-testid="level-chart" />
}
