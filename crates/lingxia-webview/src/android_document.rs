//! Exact-key correlation between Android's queued loader and page-start callback.
use crate::TrustedLoadIntent;
use std::sync::Mutex;

#[derive(Default)]
pub(crate) struct AndroidTrustedLoad {
    pending: Mutex<Option<(u64, TrustedLoadIntent)>>,
}

impl std::fmt::Debug for AndroidTrustedLoad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidTrustedLoad").finish_non_exhaustive()
    }
}

impl AndroidTrustedLoad {
    #[cfg_attr(feature = "servo", allow(dead_code))]
    pub(crate) fn arm(&self, key: u64, intent: TrustedLoadIntent) -> Option<TrustedLoadIntent> {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace((key, intent))
            .map(|(_, intent)| intent)
    }

    pub(crate) fn take(&self, key: u64) -> Option<TrustedLoadIntent> {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        if pending
            .as_ref()
            .is_some_and(|(expected, _)| *expected == key)
        {
            pending.take().map(|(_, intent)| intent)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_loader_callback_consumes_the_intent_once() {
        let pending = AndroidTrustedLoad::default();
        let intent = TrustedLoadIntent::new(1);
        assert!(pending.arm(41, intent).is_none());
        assert!(pending.take(40).is_none());
        assert!(pending.take(0).is_none());
        assert!(pending.take(41) == Some(intent));
        assert!(pending.take(41).is_none());
    }

    #[test]
    fn replacement_retires_the_previous_loader_key() {
        let pending = AndroidTrustedLoad::default();
        let first = TrustedLoadIntent::new(1);
        let second = TrustedLoadIntent::new(2);
        pending.arm(41, first);
        assert!(pending.arm(42, second) == Some(first));
        assert!(pending.take(41).is_none());
        assert!(pending.take(42) == Some(second));
    }
}
