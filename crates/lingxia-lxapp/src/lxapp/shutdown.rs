use super::{LxAppError, LxApps};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Default)]
struct AdmissionState {
    preserved: Option<HashSet<String>>,
    active: usize,
    completed: bool,
}

pub(super) struct Admission {
    state: Mutex<AdmissionState>,
    active: watch::Sender<usize>,
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            state: Mutex::new(AdmissionState::default()),
            active: watch::channel(0).0,
        }
    }
}

pub(super) struct Permit(Arc<Admission>);

impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.active -= 1;
        self.0.active.send_replace(state.active);
    }
}

impl Admission {
    pub(super) fn enter(self: &Arc<Self>, appid: &str) -> Result<Option<Permit>, LxAppError> {
        let mut state = self.state.lock().unwrap();
        if let Some(preserved) = &state.preserved {
            if preserved.contains(appid) {
                return Ok(None);
            }
            return Err(LxAppError::Runtime(
                "LxApp shutdown in progress; opening is blocked".into(),
            ));
        }
        state.active += 1;
        self.active.send_replace(state.active);
        Ok(Some(Permit(self.clone())))
    }

    fn block(&self, preserved: HashSet<String>) -> Result<(), LxAppError> {
        let mut state = self.state.lock().unwrap();
        if state
            .preserved
            .as_ref()
            .is_some_and(|existing| existing != &preserved)
        {
            return Err(LxAppError::Runtime(
                "A different LxApp shutdown is already in progress".into(),
            ));
        }
        if state.preserved.is_none() {
            state.completed = false;
        }
        state.preserved = Some(preserved);
        Ok(())
    }

    fn resume(&self) -> Result<(), LxAppError> {
        let mut state = self.state.lock().unwrap();
        if state.preserved.is_some() && !state.completed {
            return Err(LxAppError::Runtime(
                "LxApp shutdown has not completed".into(),
            ));
        }
        state.preserved = None;
        Ok(())
    }
}

impl LxApps {
    async fn drain_apps(&self) -> Result<(), LxAppError> {
        let mut active = self.admission.active.subscribe();
        active
            .wait_for(|count| *count == 0)
            .await
            .map_err(|_| LxAppError::Runtime("LxApp admission channel closed".into()))?;
        // Creation/open permits acquired before block() have now finished. No
        // new instance can escape this snapshot, including a retained recall Arc.
        let preserved = self
            .admission
            .state
            .lock()
            .unwrap()
            .preserved
            .clone()
            .ok_or_else(|| LxAppError::Runtime("LxApp shutdown barrier is not held".into()))?;
        let apps: Vec<_> = self
            .instances
            .lock()
            .unwrap()
            .values()
            .filter(|app| !preserved.contains(&app.appid))
            .cloned()
            .collect();
        for app in &apps {
            self.retire_instance(app)?;
        }
        for app in &apps {
            let mut stopped = app.logic_contexts.subscribe();
            stopped
                .wait_for(|count| *count == 0)
                .await
                .map_err(|_| LxAppError::Runtime("Logic termination channel closed".into()))?;
        }
        self.instances
            .lock()
            .unwrap()
            .retain(|_, app| preserved.contains(&app.appid));
        self.admission.state.lock().unwrap().completed = true;
        Ok(())
    }

    async fn wait_shutdown(&self, timeout: std::time::Duration) -> Result<(), LxAppError> {
        tokio::time::timeout(timeout, self.drain_apps())
            .await
            .map_err(|_| {
                LxAppError::Runtime(
                    "Timed out shutting down LxApps; admission remains blocked".into(),
                )
            })?
    }
}

/// Block new opens/recalls immediately, then fully shut down all instances except
/// the caller's preserved app ids. Cancellation and timeout keep admission closed
/// and instances tracked. The caller serializes shutdown and explicit resumption.
pub fn shutdown_lxapps_except(
    preserved_app_ids: Vec<String>,
) -> Result<impl std::future::Future<Output = Result<(), LxAppError>> + Send + 'static, LxAppError>
{
    block_lxapp_admission(preserved_app_ids)?;
    drain_lxapps()
}

/// Synchronously close admission without creating or dropping an async operation.
pub fn block_lxapp_admission(preserved_app_ids: Vec<String>) -> Result<(), LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".into()))?;
    manager
        .admission
        .block(preserved_app_ids.into_iter().collect())
}

/// Drain the instances covered by an existing admission barrier. Dropping this
/// future keeps the barrier closed; the next drain resumes tracked shutdowns.
pub fn drain_lxapps()
-> Result<impl std::future::Future<Output = Result<(), LxAppError>> + Send + 'static, LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".into()))?;
    Ok(async move {
        manager
            .wait_shutdown(std::time::Duration::from_secs(5))
            .await
    })
}

/// Release the startup barrier after the caller has completed its operation.
pub fn resume_lxapp_admission() -> Result<(), LxAppError> {
    let manager = super::runtime_registry::get_lxapps_manager()
        .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".into()))?;
    manager.admission.resume()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appservice::LxAppWorkers;
    use crate::lxapp::{Channel, LxAppStartupOptions, register_synthetic_lxapp};
    use lingxia_platform::Platform;
    use std::time::Duration;

    fn manager() -> LxApps {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let platform = Platform::new(
            root.join("data").display().to_string(),
            root.join("cache").display().to_string(),
            "en-US".to_string(),
        )
        .unwrap();
        LxApps::new(platform, LxAppWorkers::init(1), 2)
    }

    #[tokio::test]
    async fn admission_is_closed_without_polling_a_drain() {
        #[cfg(target_vendor = "apple")]
        let _host = crate::apple_host_stubs::headless_lifecycle();
        let manager = manager();
        let id = format!("app.block-only.{}", uuid::Uuid::new_v4());
        register_synthetic_lxapp(&id);
        let app = manager.ensure_lxapp(id.clone(), Channel::Release).unwrap();
        manager.admission.block(HashSet::new()).unwrap();
        let unpolled = manager.drain_apps();
        assert!(manager.ensure_lxapp(id.clone(), Channel::Release).is_err());
        assert!(app.open(LxAppStartupOptions::default()).is_err());
        assert!(!app.session.is_cancelled());
        assert!(manager.admission.resume().is_err());
        drop(unpolled);
        assert!(manager.ensure_lxapp(id, Channel::Release).is_err());
        manager.wait_shutdown(Duration::from_secs(1)).await.unwrap();
        assert!(app.session.is_retired());
        manager.admission.resume().unwrap();
    }

    #[tokio::test]
    async fn cancelled_shutdown_and_timeout_keep_removed_instances_and_admission_closed() {
        #[cfg(target_vendor = "apple")]
        let _host = crate::apple_host_stubs::headless_lifecycle();
        let manager = manager();
        let id = format!("app.shutdown.{}", uuid::Uuid::new_v4());
        let keep = format!("app.preserved.{}", uuid::Uuid::new_v4());
        register_synthetic_lxapp(&id);
        register_synthetic_lxapp(&keep);
        let app = manager.ensure_lxapp(id.clone(), Channel::Release).unwrap();
        let preserved = manager
            .ensure_lxapp(keep.clone(), Channel::Release)
            .unwrap();
        // Model a queued/running worker whose real ACK has not arrived yet.
        app.logic_contexts.send_replace(1);
        manager
            .admission
            .block(HashSet::from([keep.clone()]))
            .unwrap();
        {
            let shutdown = manager.drain_apps();
            tokio::pin!(shutdown);
            std::future::poll_fn(|cx| {
                assert!(shutdown.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
        }
        assert!(!manager.lxapps.contains_key(&id));
        assert!(
            manager
                .instances
                .lock()
                .unwrap()
                .contains_key(&app.session_id())
        );
        assert!(manager.ensure_lxapp(id.clone(), Channel::Release).is_err());
        assert!(app.open(LxAppStartupOptions::default()).is_err());
        assert!(manager.admission.resume().is_err());
        assert!(
            manager
                .wait_shutdown(Duration::from_millis(10))
                .await
                .is_err()
        );
        assert!(!preserved.session.is_cancelled());
        assert!(Arc::ptr_eq(
            &preserved,
            &manager.ensure_lxapp(keep, Channel::Release).unwrap()
        ));
        // A retry must still wait for the same removed instance.
        app.logic_contexts.send_replace(0);
        manager.wait_shutdown(Duration::from_secs(1)).await.unwrap();
        manager.admission.resume().unwrap();
        let replacement = manager.ensure_lxapp(id, Channel::Release).unwrap();
        assert_ne!(replacement.session_id(), app.session_id());
    }

    #[test]
    fn recreate_prunes_the_replaced_instance_once_its_logic_acknowledges() {
        #[cfg(target_vendor = "apple")]
        let _host = crate::apple_host_stubs::headless_lifecycle();
        let manager = manager();
        let id = format!("app.recreated.{}", uuid::Uuid::new_v4());
        let other = format!("app.other.{}", uuid::Uuid::new_v4());
        register_synthetic_lxapp(&id);
        register_synthetic_lxapp(&other);
        let tracked = |app: &Arc<crate::LxApp>| {
            manager
                .instances
                .lock()
                .unwrap()
                .get(&app.session_id())
                .is_some_and(|tracked| Arc::ptr_eq(tracked, app))
        };
        let old = manager.ensure_lxapp(id.clone(), Channel::Release).unwrap();
        // The old Logic ACK is still pending when the replacement is created.
        old.logic_contexts.send_replace(1);
        let replacement = manager.recreate_lxapp(id, Channel::Release).unwrap();
        assert!(tracked(&old));
        old.logic_contexts.send_replace(0);
        manager.ensure_lxapp(other, Channel::Release).unwrap();
        assert!(
            !tracked(&old),
            "a replaced instance must not outlive its ACK"
        );
        assert!(tracked(&replacement));
    }

    #[tokio::test]
    async fn shutdown_waits_for_admitted_creation_before_taking_its_snapshot() {
        #[cfg(target_vendor = "apple")]
        let _host = crate::apple_host_stubs::headless_lifecycle();
        let manager = manager();
        let id = format!("app.admitted.{}", uuid::Uuid::new_v4());
        register_synthetic_lxapp(&id);
        let permit = manager.admission.enter(&id).unwrap();
        manager.admission.block(HashSet::new()).unwrap();
        {
            let shutdown = manager.drain_apps();
            tokio::pin!(shutdown);
            std::future::poll_fn(|cx| {
                assert!(shutdown.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
        }
        // Finish the creation that acquired its permit before the barrier.
        let app = manager
            .ensure_lxapp_with_session_class(
                id,
                Channel::Release,
                super::super::AppSessionClass::StandardApp,
            )
            .unwrap();
        drop(permit);
        manager.wait_shutdown(Duration::from_secs(1)).await.unwrap();
        assert!(app.session.is_cancelled());
        assert!(!manager.lxapps.contains_key(&app.appid));
        manager.admission.resume().unwrap();
    }
}
