// TanStack Query hooks over the backend read API.

import { useQuery } from '@tanstack/react-query'
import { fetchObservations, fetchStations, type ObservationParams } from '../lib/api'

export function useStations() {
  return useQuery({ queryKey: ['stations'], queryFn: fetchStations })
}

export function useObservations(stationId: number | undefined, params: ObservationParams = {}) {
  return useQuery({
    queryKey: ['observations', stationId, params],
    queryFn: () => fetchObservations(stationId as number, params),
    enabled: stationId != null,
  })
}
