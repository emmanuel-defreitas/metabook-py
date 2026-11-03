"use client";

import { useQuery } from "@tanstack/react-query";
import { listLibrary } from "@/lib/api/client";

export const LIBRARY_KEY = ["library"] as const;

export function useLibrary() {
  return useQuery({ queryKey: LIBRARY_KEY, queryFn: ({ signal }) => listLibrary(signal) });
}
