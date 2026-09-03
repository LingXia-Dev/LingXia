export type ViewLeasePhase = "none" | "granted" | "accept-sent" | "active" | "revoked" | "expired";

export interface ViewLeaseState {
  phase: ViewLeasePhase;
  leaseId?: string;
  sequence: number;
  durationMs: number;
  deadlineTickMs?: number;
}

export function emptyViewLease(): ViewLeaseState {
  return { phase: "none", sequence: 0, durationMs: 0 };
}

export function viewApplyGrant(
  leaseId: string,
  sequence: number,
  durationMs: number,
  nowMs: number
): ViewLeaseState {
  return {
    phase: "granted",
    leaseId,
    sequence,
    durationMs,
    deadlineTickMs: nowMs + durationMs,
  };
}

export function viewAcceptLease(state: ViewLeaseState): ViewLeaseState | null {
  if (state.phase !== "granted" || !state.leaseId) return null;
  return { ...state, phase: "accept-sent" };
}

export function viewMarkActive(state: ViewLeaseState): ViewLeaseState {
  return { ...state, phase: "active", deadlineTickMs: undefined };
}

export function viewCanShowFallback(state: ViewLeaseState, nowMs: number): boolean {
  if (state.phase === "none" || state.phase === "granted" || state.phase === "revoked" || state.phase === "expired") {
    return true;
  }
  if (state.phase === "accept-sent") {
    return state.deadlineTickMs !== undefined && nowMs >= state.deadlineTickMs;
  }
  if (state.phase === "active") return false;
  return false;
}
