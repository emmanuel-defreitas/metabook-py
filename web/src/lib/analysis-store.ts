// Hand-off for a freshly produced analysis that has no saved record id
// (storage disabled on the API). Lives for the tab session only.

import type { Analysis } from "./api/types";

let pending: Analysis | null = null;

export function stashAnalysis(analysis: Analysis) {
  pending = analysis;
}

export function takeStashedAnalysis(): Analysis | null {
  return pending;
}
