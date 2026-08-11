use crate::{
    PinCollection, PinMutation, ShellPinTarget, ShellResult, ShellSidebarAction,
    ShellSidebarActionUpdate, ShellStore, ShellWindowState, SidebarActionCollection, SidebarChrome,
    WindowFrame,
};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSnapshot {
    pub sidebar_actions: SidebarActionCollection,
    pub pins: PinCollection,
}

pub struct ShellManager {
    store: ShellStore,
    state: Mutex<ShellSnapshot>,
    window_state: Mutex<ShellWindowState>,
}

impl ShellManager {
    pub fn open(root: impl Into<PathBuf>) -> ShellResult<Self> {
        let store = ShellStore::new(root);
        let state = ShellSnapshot {
            sidebar_actions: SidebarActionCollection::default(),
            pins: store.load_pins_recovering(),
        };
        let window_state = store.load_window_state();
        Ok(Self {
            store,
            state: Mutex::new(state),
            window_state: Mutex::new(window_state),
        })
    }

    pub fn snapshot(&self) -> ShellSnapshot {
        self.lock().clone()
    }

    pub fn sidebar_chrome(&self) -> SidebarChrome {
        self.lock_window_state().sidebar
    }

    pub fn set_sidebar_chrome(&self, chrome: SidebarChrome) -> ShellResult<()> {
        let mut current = self.lock_window_state();
        let mut next = *current;
        next.sidebar = chrome.normalized();
        if next == *current {
            return Ok(());
        }
        self.store.save_window_state(&next)?;
        *current = next;
        Ok(())
    }

    pub fn window_frame(&self) -> Option<WindowFrame> {
        self.lock_window_state().window
    }

    pub fn set_window_frame(&self, frame: WindowFrame) -> ShellResult<()> {
        if !frame.valid() {
            return Err(crate::ShellError::InvalidState(
                "window frame must be finite with positive dimensions".to_string(),
            ));
        }
        let mut current = self.lock_window_state();
        let mut next = *current;
        next.window = Some(frame);
        if next == *current {
            return Ok(());
        }
        self.store.save_window_state(&next)?;
        *current = next;
        Ok(())
    }

    pub fn replace_sidebar_actions(
        &self,
        items: Vec<ShellSidebarAction>,
    ) -> ShellResult<ShellSnapshot> {
        self.mutate_sidebar_actions(|state| state.replace(items))
    }

    pub fn update_sidebar_action(
        &self,
        id: &str,
        patch: ShellSidebarActionUpdate,
    ) -> ShellResult<ShellSnapshot> {
        self.mutate_sidebar_actions(|state| state.update(id, patch))
    }

    pub fn remove_sidebar_action(&self, id: &str) -> ShellResult<ShellSnapshot> {
        self.mutate_sidebar_actions(|state| state.remove(id))
    }

    pub fn clear_sidebar_actions(&self) -> ShellResult<ShellSnapshot> {
        self.mutate_sidebar_actions(|state| {
            state.clear();
            Ok(())
        })
    }

    pub fn commit_sidebar_actions(
        &self,
        expected_generation: u64,
        next: SidebarActionCollection,
    ) -> ShellResult<ShellSnapshot> {
        let mut state = self.lock();
        let actual = state.sidebar_actions.generation();
        if actual != expected_generation {
            return Err(crate::ShellError::ConcurrentMutation {
                expected: expected_generation,
                actual,
            });
        }
        let mut snapshot = state.clone();
        snapshot.sidebar_actions = next;
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

    fn mutate_sidebar_actions(
        &self,
        mutate: impl FnOnce(&mut SidebarActionCollection) -> ShellResult<()>,
    ) -> ShellResult<ShellSnapshot> {
        let current = self.snapshot();
        let mut next = current.sidebar_actions.clone();
        mutate(&mut next)?;
        self.commit_sidebar_actions(current.sidebar_actions.generation(), next)
    }

    fn lock(&self) -> MutexGuard<'_, ShellSnapshot> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn lock_window_state(&self) -> MutexGuard<'_, ShellWindowState> {
        self.window_state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
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
            .replace_sidebar_actions(vec![ShellSidebarAction {
                id: "chat".to_string(),
                placement: crate::SidebarActionPlacement::Footer,
                label: "Chat".to_string(),
                icon: "icons/chat.svg".to_string(),
                disabled: false,
            }])
            .unwrap();
        let before = manager.snapshot();

        let result = manager.replace_sidebar_actions(vec![ShellSidebarAction {
            id: "".to_string(),
            placement: crate::SidebarActionPlacement::Footer,
            label: "Broken".to_string(),
            icon: "icons/broken.svg".to_string(),
            disabled: false,
        }]);

        assert_eq!(result, Err(ShellError::EmptySidebarActionId));
        assert_eq!(manager.snapshot(), before);
    }

    #[test]
    fn sidebar_actions_are_process_local() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ShellManager::open(dir.path()).unwrap();
        manager
            .replace_sidebar_actions(vec![ShellSidebarAction {
                id: "chat".to_string(),
                placement: crate::SidebarActionPlacement::Footer,
                label: "Chat".to_string(),
                icon: "icons/chat.svg".to_string(),
                disabled: false,
            }])
            .unwrap();

        let reopened = ShellManager::open(dir.path()).unwrap();
        assert!(!reopened.snapshot().sidebar_actions.declared());
        assert!(reopened.snapshot().sidebar_actions.items().is_empty());
    }

    #[test]
    fn sidebar_and_window_updates_preserve_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ShellManager::open(dir.path()).unwrap();
        let frame = WindowFrame::new(10.0, 20.0, 1200.0, 800.0).unwrap();

        manager.set_window_frame(frame).unwrap();
        manager
            .set_sidebar_chrome(SidebarChrome::with_expanded(false, 248.0))
            .unwrap();

        let reopened = ShellManager::open(dir.path()).unwrap();
        assert_eq!(reopened.window_frame(), Some(frame));
        assert_eq!(
            reopened.sidebar_chrome(),
            SidebarChrome::with_expanded(false, 248.0)
        );
    }
}
