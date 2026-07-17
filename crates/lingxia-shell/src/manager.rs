use crate::{
    ActivatorCollection, PinCollection, PinMutation, ShellActivator, ShellActivatorUpdate,
    ShellPinTarget, ShellResult, ShellStore,
};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub activators: ActivatorCollection,
    pub pins: PinCollection,
}

pub struct ShellManager {
    store: ShellStore,
    state: Mutex<ShellSnapshot>,
}

impl ShellManager {
    pub fn open(root: impl Into<PathBuf>) -> ShellResult<Self> {
        let store = ShellStore::new(root);
        let state = ShellSnapshot {
            activators: ActivatorCollection::default(),
            pins: store.load_pins_recovering(),
        };
        Ok(Self {
            store,
            state: Mutex::new(state),
        })
    }

    pub fn snapshot(&self) -> ShellSnapshot {
        self.lock().clone()
    }

    pub fn replace_activators(&self, items: Vec<ShellActivator>) -> ShellResult<ShellSnapshot> {
        self.mutate_activators(|state| state.replace(items))
    }

    pub fn update_activator(
        &self,
        id: &str,
        patch: ShellActivatorUpdate,
    ) -> ShellResult<ShellSnapshot> {
        self.mutate_activators(|state| state.update(id, patch))
    }

    pub fn remove_activator(&self, id: &str) -> ShellResult<ShellSnapshot> {
        self.mutate_activators(|state| state.remove(id))
    }

    pub fn clear_activators(&self) -> ShellResult<ShellSnapshot> {
        self.mutate_activators(|state| {
            state.clear();
            Ok(())
        })
    }

    pub fn commit_activators(
        &self,
        expected_generation: u64,
        next: ActivatorCollection,
    ) -> ShellResult<ShellSnapshot> {
        let mut state = self.lock();
        let actual = state.activators.generation();
        if actual != expected_generation {
            return Err(crate::ShellError::ConcurrentMutation {
                expected: expected_generation,
                actual,
            });
        }
        let mut snapshot = state.clone();
        snapshot.activators = next;
        *state = snapshot;
        Ok(state.clone())
    }

    pub fn pin(&self, target: ShellPinTarget) -> ShellResult<(PinMutation, ShellSnapshot)> {
        let mut state = self.lock();
        let mut next = state.clone();
        let mutation = next.pins.pin(target)?;
        if mutation == PinMutation::Changed {
            self.store.save_pins(&next.pins)?;
            *state = next;
        }
        Ok((mutation, state.clone()))
    }

    pub fn unpin(&self, target: &ShellPinTarget) -> ShellResult<(PinMutation, ShellSnapshot)> {
        let mut state = self.lock();
        let mut next = state.clone();
        let mutation = next.pins.unpin(target);
        if mutation == PinMutation::Changed {
            self.store.save_pins(&next.pins)?;
            *state = next;
        }
        Ok((mutation, state.clone()))
    }

    pub fn commit_pins(
        &self,
        expected: &PinCollection,
        next: PinCollection,
    ) -> ShellResult<ShellSnapshot> {
        let mut state = self.lock();
        if state.pins != *expected {
            return Err(crate::ShellError::ConcurrentPinMutation);
        }
        let mut snapshot = state.clone();
        snapshot.pins = next;
        self.store.save_pins(&snapshot.pins)?;
        *state = snapshot;
        Ok(state.clone())
    }

    fn mutate_activators(
        &self,
        mutate: impl FnOnce(&mut ActivatorCollection) -> ShellResult<()>,
    ) -> ShellResult<ShellSnapshot> {
        let current = self.snapshot();
        let mut next = current.activators.clone();
        mutate(&mut next)?;
        self.commit_activators(current.activators.generation(), next)
    }

    fn lock(&self) -> MutexGuard<'_, ShellSnapshot> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellError;

    #[test]
    fn failed_replacement_does_not_change_memory() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ShellManager::open(dir.path()).unwrap();
        manager
            .replace_activators(vec![ShellActivator {
                id: "chat".to_string(),
                label: "Chat".to_string(),
                icon: "icons/chat.svg".to_string(),
                disabled: false,
            }])
            .unwrap();
        let before = manager.snapshot();

        let result = manager.replace_activators(vec![ShellActivator {
            id: "".to_string(),
            label: "Broken".to_string(),
            icon: "icons/broken.svg".to_string(),
            disabled: false,
        }]);

        assert_eq!(result, Err(ShellError::EmptyActivatorId));
        assert_eq!(manager.snapshot(), before);
    }

    #[test]
    fn activators_are_process_local() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ShellManager::open(dir.path()).unwrap();
        manager
            .replace_activators(vec![ShellActivator {
                id: "chat".to_string(),
                label: "Chat".to_string(),
                icon: "icons/chat.svg".to_string(),
                disabled: false,
            }])
            .unwrap();

        let reopened = ShellManager::open(dir.path()).unwrap();
        assert!(!reopened.snapshot().activators.declared());
        assert!(reopened.snapshot().activators.items().is_empty());
    }
}
