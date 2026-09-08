use super::types::{NativeRootLeaseMessage, RootRef};

/// Default lease length. Protocol-fixed; apps cannot change it.
pub const DEFAULT_LEASE_DURATION_MS: u64 = 8_000;
pub const NEGOTIATION_TIMEOUT_MS: u64 = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeasePhase {
    None,
    Granted,
    AcceptSent,
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseState {
    pub phase: LeasePhase,
    pub lease_id: Option<String>,
    pub sequence: u64,
    pub duration_ms: u64,
    /// Tick the current grant/accept deadline is measured from (host or view).
    pub deadline_tick_ms: Option<u64>,
    pub displayable: bool,
}

impl Default for LeaseState {
    fn default() -> Self {
        Self {
            phase: LeasePhase::None,
            lease_id: None,
            sequence: 0,
            duration_ms: DEFAULT_LEASE_DURATION_MS,
            deadline_tick_ms: None,
            displayable: false,
        }
    }
}

pub fn host_grant_lease(
    root: &RootRef,
    lease_id: String,
    now_ms: u64,
) -> (LeaseState, NativeRootLeaseMessage) {
    let state = LeaseState {
        phase: LeasePhase::Granted,
        lease_id: Some(lease_id.clone()),
        sequence: 1,
        duration_ms: DEFAULT_LEASE_DURATION_MS,
        deadline_tick_ms: Some(now_ms + DEFAULT_LEASE_DURATION_MS),
        displayable: false,
    };
    let message = NativeRootLeaseMessage::LeaseGranted {
        root: root.clone(),
        lease_id,
        sequence: 1,
        lease_duration_ms: DEFAULT_LEASE_DURATION_MS,
    };
    (state, message)
}

pub fn host_on_accept(
    state: &mut LeaseState,
    root: &RootRef,
    lease_id: &str,
    sequence: u64,
) -> Option<NativeRootLeaseMessage> {
    if state.phase != LeasePhase::Granted {
        return None;
    }
    if state.lease_id.as_deref() != Some(lease_id) || state.sequence != sequence {
        return None;
    }
    state.phase = LeasePhase::Active;
    state.displayable = true;
    Some(NativeRootLeaseMessage::LeaseActive {
        root: root.clone(),
        lease_id: lease_id.to_string(),
        sequence,
    })
}

pub fn host_on_renew(
    state: &LeaseState,
    root: &RootRef,
    lease_id: &str,
    sequence: u64,
) -> Option<NativeRootLeaseMessage> {
    if state.phase != LeasePhase::Active {
        return None;
    }
    if state.lease_id.as_deref() != Some(lease_id) {
        return None;
    }
    Some(NativeRootLeaseMessage::LeaseRenewGranted {
        root: root.clone(),
        lease_id: lease_id.to_string(),
        sequence,
        lease_duration_ms: state.duration_ms,
    })
}

pub fn host_on_renew_accept(
    state: &mut LeaseState,
    lease_id: &str,
    sequence: u64,
    now_ms: u64,
) -> bool {
    if state.phase != LeasePhase::Active {
        return false;
    }
    if state.lease_id.as_deref() != Some(lease_id) {
        return false;
    }
    state.sequence = sequence;
    state.deadline_tick_ms = Some(now_ms + state.duration_ms);
    true
}

pub fn host_revoke_lease(
    state: &mut LeaseState,
    root: &RootRef,
    reason: &str,
) -> NativeRootLeaseMessage {
    let lease_id = state.lease_id.clone().unwrap_or_default();
    let sequence = state.sequence;
    state.phase = LeasePhase::Revoked;
    state.displayable = false;
    NativeRootLeaseMessage::LeaseRevoked {
        root: root.clone(),
        lease_id,
        sequence,
        reason: reason.to_string(),
    }
}

pub fn host_tick_lease(state: &mut LeaseState, now_ms: u64) -> bool {
    if let Some(deadline) = state.deadline_tick_ms
        && now_ms >= deadline
        && matches!(
            state.phase,
            LeasePhase::Granted | LeasePhase::Active | LeasePhase::AcceptSent
        )
    {
        state.phase = LeasePhase::Expired;
        state.displayable = false;
        return true;
    }
    false
}

pub fn host_can_display(state: &LeaseState) -> bool {
    state.phase == LeasePhase::Active && state.displayable
}

pub fn view_on_grant(
    _root: &RootRef,
    lease_id: &str,
    sequence: u64,
    duration_ms: u64,
    now_ms: u64,
    already_accepted: bool,
) -> LeaseState {
    if already_accepted {
        return LeaseState {
            phase: LeasePhase::AcceptSent,
            lease_id: Some(lease_id.to_string()),
            sequence,
            duration_ms,
            deadline_tick_ms: Some(now_ms + duration_ms),
            displayable: false,
        };
    }
    LeaseState {
        phase: LeasePhase::Granted,
        lease_id: Some(lease_id.to_string()),
        sequence,
        duration_ms,
        deadline_tick_ms: Some(now_ms + duration_ms),
        displayable: false,
    }
}

pub fn view_send_accept(state: &mut LeaseState, root: &RootRef) -> Option<NativeRootLeaseMessage> {
    if state.phase != LeasePhase::Granted {
        return None;
    }
    let lease_id = state.lease_id.clone()?;
    state.phase = LeasePhase::AcceptSent;
    Some(NativeRootLeaseMessage::LeaseAccept {
        root: root.clone(),
        lease_id,
        sequence: state.sequence,
    })
}

pub fn view_on_renew_granted(
    state: &mut LeaseState,
    root: &RootRef,
    lease_id: &str,
    sequence: u64,
    duration_ms: u64,
    now_ms: u64,
) -> Option<NativeRootLeaseMessage> {
    if state.lease_id.as_deref() != Some(lease_id) {
        return None;
    }
    state.deadline_tick_ms = Some(now_ms + duration_ms);
    Some(NativeRootLeaseMessage::LeaseRenewAccept {
        root: root.clone(),
        lease_id: lease_id.to_string(),
        sequence,
    })
}

/// Fallback is allowed only when the View never sent accept, or the local
/// grant deadline has expired after accept.
pub fn view_can_show_fallback(state: &LeaseState, now_ms: u64) -> bool {
    match state.phase {
        LeasePhase::None | LeasePhase::Revoked | LeasePhase::Expired => true,
        LeasePhase::Granted => true,
        LeasePhase::AcceptSent | LeasePhase::Active => state
            .deadline_tick_ms
            .is_some_and(|deadline| now_ms >= deadline),
    }
}
