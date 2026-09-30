use super::*;
use crate::appservice::ServiceMessage;
use std::sync::mpsc;

struct Fixture {
    manager: LxApps,
    old: Arc<LxApp>,
    workers: Arc<LxAppWorkers>,
    messages: mpsc::Receiver<ServiceMessage>,
    _root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let runtime = Platform::new(
            root.path().join("data").display().to_string(),
            root.path().join("cache").display().to_string(),
            "en-US".into(),
        )
        .unwrap();
        let (workers, messages) = LxAppWorkers::manual_for_test();
        let manager = LxApps::new(runtime, workers.clone(), 1);
        let appid = format!("app.lingxia.restart.{}", Uuid::new_v4());
        register_synthetic_lxapp(&appid);
        let old = manager
            .ensure_lxapp(appid.clone(), Channel::Release)
            .unwrap();
        old.config().logic = Some(LxAppLogicEntry::Enabled(true));
        workers.create_app_svc(old.clone()).unwrap();
        assert!(matches!(
            messages.try_recv().unwrap(),
            ServiceMessage::CreateAppSvc { .. }
        ));
        manager.lxapp_stack.lock().unwrap().push_back(appid);
        Self {
            manager,
            old,
            workers,
            messages,
            _root: root,
        }
    }

    fn take_ack(&self) -> oneshot::Sender<()> {
        let ServiceMessage::TerminateAppSvc { lxapp, ack_tx, .. } =
            self.messages.try_recv().unwrap()
        else {
            panic!("expected a worker termination request");
        };
        assert!(Arc::ptr_eq(&lxapp, &self.old));
        ack_tx
    }

    fn ensure_replacement(&self) -> Arc<LxApp> {
        let app = self
            .manager
            .ensure_lxapp(self.old.appid.clone(), Channel::Release)
            .unwrap();
        assert_ne!(app.session_id(), self.old.session_id());
        assert!(!app.session.is_retired());
        app.config().logic = Some(LxAppLogicEntry::Enabled(true));
        app
    }
}

fn assert_retired_removed(manager: &LxApps, old: &LxApp) {
    assert!(old.session.is_retired());
    assert!(!manager.lxapps.contains_key(&old.appid));
    assert!(!manager.lxapp_stack.lock().unwrap().contains(&old.appid));
    assert!(!manager.session_transition_locks.contains_key(&old.appid));
}

#[tokio::test]
async fn recreate_waits_for_worker_release_and_joins_pending_termination() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    let ack = f.take_ack();
    let joined = f.workers.terminate_app_svc(f.old.clone()).unwrap();
    assert!(
        f.messages.try_recv().is_err(),
        "repeated termination must join the same request"
    );

    // The context count can reach zero before the worker ACK is processed.
    f.old.logic_contexts.send_replace(0);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    assert!(Arc::ptr_eq(
        f.manager.lxapps.get(&f.old.appid).unwrap().value(),
        &f.old
    ));
    assert!(
        !f.manager
            .session_transition_locks
            .contains_key(&f.old.appid)
    );
    assert!(f.old.presentation_open_lock.try_lock().is_ok());

    ack.send(()).unwrap();
    let replacement = time::timeout(Duration::from_secs(1), restart)
        .await
        .unwrap()
        .unwrap();
    joined.wait().await.unwrap();
    assert_ne!(replacement.session_id(), f.old.session_id());
    assert!(
        !f.manager
            .instances
            .lock()
            .unwrap()
            .contains_key(&f.old.session_id())
    );
    assert!(
        f.manager
            .instances
            .lock()
            .unwrap()
            .contains_key(&replacement.session_id())
    );
    replacement.config().logic = Some(LxAppLogicEntry::Enabled(true));
    f.workers.create_app_svc(replacement.clone()).unwrap();
    let ServiceMessage::CreateAppSvc { lxapp } = f.messages.try_recv().unwrap() else {
        panic!("replacement must acquire the released worker");
    };
    assert!(Arc::ptr_eq(&lxapp, &replacement));
}

#[tokio::test]
async fn failed_restart_removes_retired_instance_when_ack_closes() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    drop(f.take_ack());
    let error = time::timeout(Duration::from_secs(1), restart)
        .await
        .unwrap()
        .err()
        .unwrap();
    assert!(error.to_string().contains("ACK channel closed"));
    assert_retired_removed(&f.manager, &f.old);
    let replacement = f.ensure_replacement();
    assert!(
        matches!(
            f.workers.create_app_svc(replacement),
            Err(LxAppError::ResourceExhausted(_))
        ),
        "a missing ACK must keep the worker quarantined"
    );
}

#[tokio::test]
async fn failed_restart_can_reopen_after_timeout_and_late_ack() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    let ack = f.take_ack();
    let error = time::timeout(Duration::from_secs(10), restart)
        .await
        .unwrap()
        .err()
        .unwrap();
    assert!(error.to_string().contains("ACK timed out"));
    assert_retired_removed(&f.manager, &f.old);
    let replacement = f.ensure_replacement();
    assert!(matches!(
        f.workers.create_app_svc(replacement.clone()),
        Err(LxAppError::ResourceExhausted(_))
    ));

    f.old.logic_contexts.send_replace(0);
    ack.send(()).unwrap();
    time::timeout(Duration::from_secs(1), async {
        loop {
            match f.workers.create_app_svc(replacement.clone()) {
                Ok(()) => break,
                Err(LxAppError::ResourceExhausted(_)) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected restart error: {error}"),
            }
        }
    })
    .await
    .expect("late ACK must make a later open possible");
}

#[tokio::test]
async fn failed_restart_removes_retired_instance_when_dispatch_fails() {
    let Fixture {
        manager,
        old,
        messages,
        _root,
        ..
    } = Fixture::new();
    drop(messages);
    let error = manager
        .recreate_lxapp(old.appid.clone(), Channel::Release, old.session_id())
        .await
        .err()
        .unwrap();
    assert!(matches!(error, LxAppError::ChannelError(_)));
    assert_retired_removed(&manager, &old);
    let replacement = manager
        .ensure_lxapp(old.appid.clone(), Channel::Release)
        .unwrap();
    assert_ne!(replacement.session_id(), old.session_id());
    assert!(!replacement.session.is_retired());
}

#[tokio::test]
async fn restart_completion_cannot_remove_a_session_replaced_during_wait() {
    #[cfg(target_vendor = "apple")]
    let _host = crate::apple_host_stubs::headless_lifecycle();
    for acknowledge in [true, false] {
        let f = Fixture::new();
        let restart =
            f.manager
                .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
        tokio::pin!(restart);
        assert!(futures::poll!(restart.as_mut()).is_pending());
        let ack = f.take_ack();
        f.manager
            .with_session_transition(&f.old.appid, || f.manager.destroy_lxapp(&f.old.appid));
        let replacement = f.ensure_replacement();
        f.manager
            .lxapp_stack
            .lock()
            .unwrap()
            .push_back(f.old.appid.clone());
        if acknowledge {
            ack.send(()).unwrap();
        } else {
            drop(ack);
        }
        let error = time::timeout(Duration::from_secs(1), restart)
            .await
            .unwrap()
            .err()
            .unwrap();
        assert!(error.to_string().contains("replaced or removed"));
        assert!(Arc::ptr_eq(
            f.manager.lxapps.get(&f.old.appid).unwrap().value(),
            &replacement
        ));
        assert!(!replacement.session.is_cancelled());
        assert!(f.manager.lxapp_stack.lock().unwrap().contains(&f.old.appid));
    }
}
