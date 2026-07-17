use crate::{
    ResolvedShellActivator, ShellActivator, ShellError, ShellManager, ShellPin, ShellPinTarget,
    ShellResult,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellActivationIntent {
    pub id: String,
    pub generation: u64,
}

pub trait ShellHost: Send + Sync + 'static {
    fn resolve_activators(
        &self,
        items: &[ShellActivator],
    ) -> ShellResult<Vec<ResolvedShellActivator>>;

    fn apply_activators(&self, items: &[ResolvedShellActivator]) -> ShellResult<()>;

    fn apply_pins(&self, items: &[ShellPin]) -> ShellResult<()>;

    fn activate(&self, intent: ShellActivationIntent) -> ShellResult<()>;
}

#[derive(Clone)]
struct ActiveShell {
    manager: Arc<ShellManager>,
    host: Arc<dyn ShellHost>,
}

fn active_slot() -> &'static Mutex<Option<ActiveShell>> {
    static ACTIVE: OnceLock<Mutex<Option<ActiveShell>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(None))
}

pub fn initialize(root: impl Into<PathBuf>, host: Arc<dyn ShellHost>) -> ShellResult<()> {
    let manager = Arc::new(ShellManager::open(root)?);
    let mut active = active_slot()
        .lock()
        .map_err(|_| ShellError::Host("active shell state is poisoned".to_string()))?;
    *active = Some(ActiveShell { manager, host });
    Ok(())
}

pub fn manager() -> ShellResult<Arc<ShellManager>> {
    active_slot()
        .lock()
        .map_err(|_| ShellError::Host("active shell state is poisoned".to_string()))?
        .as_ref()
        .map(|active| active.manager.clone())
        .ok_or(ShellError::NotInitialized)
}

pub fn resolved_activators() -> ShellResult<Vec<ResolvedShellActivator>> {
    with_active(|active| {
        let snapshot = active.manager.snapshot();
        active.host.resolve_activators(snapshot.activators.items())
    })
}

pub fn apply_current_activators() -> ShellResult<Vec<ResolvedShellActivator>> {
    with_active(|active| {
        let snapshot = active.manager.snapshot();
        let resolved = active
            .host
            .resolve_activators(snapshot.activators.items())?;
        active.host.apply_activators(&resolved)?;
        Ok(resolved)
    })
}

pub fn pins() -> ShellResult<Vec<ShellPin>> {
    Ok(manager()?.snapshot().pins.items)
}

pub fn apply_current_pins() -> ShellResult<Vec<ShellPin>> {
    with_active(|active| {
        let items = active.manager.snapshot().pins.items;
        active.host.apply_pins(&items)?;
        Ok(items)
    })
}

pub fn is_pinned(target: &ShellPinTarget) -> ShellResult<bool> {
    Ok(manager()?.snapshot().pins.is_pinned(target))
}

pub fn set_pinned(target: ShellPinTarget, pinned: bool) -> ShellResult<crate::PinMutation> {
    let _mutation = pin_mutation_lock()
        .lock()
        .map_err(|_| ShellError::Host("shell Pin mutation state is poisoned".to_string()))?;
    with_active(|active| {
        let previous = active.manager.snapshot().pins;
        let (mutation, snapshot) = if pinned {
            active.manager.pin(target)?
        } else {
            active.manager.unpin(&target)?
        };
        if mutation == crate::PinMutation::Changed
            && let Err(error) = active.host.apply_pins(&snapshot.pins.items)
        {
            let _ = active.manager.commit_pins(&snapshot.pins, previous.clone());
            let _ = active.host.apply_pins(&previous.items);
            return Err(error);
        }
        Ok(mutation)
    })
}

fn pin_mutation_lock() -> &'static Mutex<()> {
    static MUTATION: OnceLock<Mutex<()>> = OnceLock::new();
    MUTATION.get_or_init(|| Mutex::new(()))
}

pub fn activate(id: &str) -> ShellResult<()> {
    let id = id.trim();
    if id.is_empty() {
        return Err(ShellError::EmptyActivatorId);
    }
    with_active(|active| {
        let snapshot = active.manager.snapshot();
        let Some(item) = snapshot
            .activators
            .items()
            .iter()
            .find(|item| item.id == id)
        else {
            return Err(ShellError::ActivatorNotFound { id: id.to_string() });
        };
        if item.disabled {
            return Err(ShellError::ActivatorDisabled { id: id.to_string() });
        }
        let intent = ShellActivationIntent {
            id: item.id.clone(),
            generation: snapshot.activators.generation(),
        };
        active.host.activate(intent)
    })?;
    let _ = apply_current_activators();
    Ok(())
}

fn with_active<T>(run: impl FnOnce(&ActiveShell) -> ShellResult<T>) -> ShellResult<T> {
    let active = {
        let slot = active_slot()
            .lock()
            .map_err(|_| ShellError::Host("active shell state is poisoned".to_string()))?;
        slot.clone().ok_or(ShellError::NotInitialized)?
    };
    run(&active)
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    *active_slot().lock().unwrap() = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[derive(Default)]
    struct TestHost {
        activated: Mutex<Vec<ShellActivationIntent>>,
        applied: Mutex<Vec<Vec<ResolvedShellActivator>>>,
        applied_pins: Mutex<Vec<Vec<ShellPin>>>,
        reject_pins: AtomicBool,
    }

    impl ShellHost for TestHost {
        fn resolve_activators(
            &self,
            items: &[ShellActivator],
        ) -> ShellResult<Vec<ResolvedShellActivator>> {
            Ok(items
                .iter()
                .map(|item| ResolvedShellActivator {
                    id: item.id.clone(),
                    label: item.label.clone(),
                    icon_path: Some(item.icon.clone()),
                    disabled: item.disabled,
                })
                .collect())
        }

        fn apply_activators(&self, items: &[ResolvedShellActivator]) -> ShellResult<()> {
            self.applied.lock().unwrap().push(items.to_vec());
            Ok(())
        }

        fn apply_pins(&self, items: &[ShellPin]) -> ShellResult<()> {
            self.applied_pins.lock().unwrap().push(items.to_vec());
            if self.reject_pins.load(Ordering::Relaxed) {
                return Err(ShellError::Host("rejected Pins".to_string()));
            }
            Ok(())
        }

        fn activate(&self, intent: ShellActivationIntent) -> ShellResult<()> {
            self.activated.lock().unwrap().push(intent);
            Ok(())
        }
    }

    #[test]
    fn stable_id_activation_routes_the_current_generation() {
        let _guard = test_guard();
        reset_for_test();
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::default());
        initialize(dir.path(), host.clone()).unwrap();
        manager()
            .unwrap()
            .replace_activators(vec![ShellActivator {
                id: "sync".to_string(),
                label: "Sync".to_string(),
                icon: "icons/sync.svg".to_string(),
                disabled: false,
            }])
            .unwrap();

        activate("sync").unwrap();

        assert_eq!(
            host.activated.lock().unwrap().as_slice(),
            &[ShellActivationIntent {
                id: "sync".to_string(),
                generation: 1,
            }]
        );
        assert_eq!(host.applied.lock().unwrap()[0][0].label, "Sync");
    }

    #[test]
    fn disabled_items_never_reach_the_host() {
        let _guard = test_guard();
        reset_for_test();
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::default());
        initialize(dir.path(), host.clone()).unwrap();
        manager()
            .unwrap()
            .replace_activators(vec![ShellActivator {
                id: "chat".to_string(),
                label: "Chat".to_string(),
                icon: "icons/chat.svg".to_string(),
                disabled: true,
            }])
            .unwrap();

        assert_eq!(
            activate("chat"),
            Err(ShellError::ActivatorDisabled {
                id: "chat".to_string()
            })
        );
        assert!(host.activated.lock().unwrap().is_empty());
    }

    #[test]
    fn pin_mutations_apply_one_mixed_order_and_reject_ninth() {
        let _guard = test_guard();
        reset_for_test();
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::default());
        initialize(dir.path(), host.clone()).unwrap();
        for index in 0..crate::MAX_SHELL_PINS {
            let target = if index % 2 == 0 {
                ShellPinTarget::Lxapp {
                    key: format!("app.{index}"),
                }
            } else {
                ShellPinTarget::Bookmark {
                    key: format!("bookmark-{index}"),
                }
            };
            set_pinned(target, true).unwrap();
        }

        assert_eq!(host.applied_pins.lock().unwrap().len(), 8);
        assert_eq!(
            set_pinned(
                ShellPinTarget::Lxapp {
                    key: "app.overflow".to_string(),
                },
                true,
            ),
            Err(ShellError::LimitReached {
                max: crate::MAX_SHELL_PINS,
            })
        );
        assert_eq!(host.applied_pins.lock().unwrap().len(), 8);
    }

    #[test]
    fn failed_pin_apply_rolls_back_memory_and_disk() {
        let _guard = test_guard();
        reset_for_test();
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(TestHost::default());
        initialize(dir.path(), host.clone()).unwrap();
        host.reject_pins.store(true, Ordering::Relaxed);

        assert_eq!(
            set_pinned(
                ShellPinTarget::Lxapp {
                    key: "app.chat".to_string(),
                },
                true,
            ),
            Err(ShellError::Host("rejected Pins".to_string()))
        );
        assert!(manager().unwrap().snapshot().pins.items.is_empty());
        assert!(
            ShellManager::open(dir.path())
                .unwrap()
                .snapshot()
                .pins
                .items
                .is_empty()
        );
    }
}
