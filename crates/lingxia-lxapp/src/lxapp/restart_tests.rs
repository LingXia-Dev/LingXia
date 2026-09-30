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
        Self::with_ack_timeout(Duration::from_secs(3))
    }

    fn with_ack_timeout(ack_timeout: Duration) -> Self {
        let root = tempfile::tempdir().unwrap();
        let runtime = Platform::new(
            root.path().join("data").display().to_string(),
            root.path().join("cache").display().to_string(),
            "en-US".to_string(),
        )
        .unwrap();
        let (workers, messages) = LxAppWorkers::manual_for_test(ack_timeout);
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

fn assert_replaced(manager: &LxApps, old: &LxApp, replacement: &Arc<LxApp>) {
    assert!(old.session.is_retired());
    assert_ne!(replacement.session_id(), old.session_id());
    assert!(!replacement.session.is_retired());
    assert!(Arc::ptr_eq(
        manager.lxapps.get(&old.appid).unwrap().value(),
        replacement
    ));
    assert!(!manager.session_transition_locks.contains_key(&old.appid));
    replacement.config().logic = Some(LxAppLogicEntry::Enabled(true));
}

fn tracked(manager: &LxApps, app: &LxApp) -> bool {
    manager
        .instances
        .lock()
        .unwrap()
        .contains_key(&app.session_id())
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
async fn restart_still_replaces_the_session_when_ack_closes() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    drop(f.take_ack());
    let replacement = time::timeout(Duration::from_secs(1), restart)
        .await
        .unwrap()
        .unwrap();
    assert_replaced(&f.manager, &f.old, &replacement);
    assert!(
        matches!(
            f.workers.create_app_svc(replacement),
            Err(LxAppError::ResourceExhausted(_))
        ),
        "a missing ACK must keep the worker quarantined"
    );
}

#[tokio::test]
async fn restart_replaces_a_hung_session_and_prunes_it_after_a_late_ack() {
    let f = Fixture::with_ack_timeout(Duration::from_millis(50));
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    let ack = f.take_ack();
    let replacement = time::timeout(Duration::from_secs(5), restart)
        .await
        .unwrap()
        .unwrap();
    assert_replaced(&f.manager, &f.old, &replacement);
    assert!(matches!(
        f.workers.create_app_svc(replacement.clone()),
        Err(LxAppError::ResourceExhausted(_))
    ));
    // The hung Logic is still live, so a shutdown drain must keep seeing it.
    f.manager.track_instance(&replacement);
    assert!(tracked(&f.manager, &f.old));

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
    f.manager.track_instance(&replacement);
    assert!(
        !tracked(&f.manager, &f.old),
        "a replaced instance must not outlive its ACK"
    );
    assert!(tracked(&f.manager, &replacement));
}

#[tokio::test]
async fn restart_still_replaces_the_session_when_dispatch_fails() {
    let Fixture {
        manager,
        old,
        messages,
        _root,
        ..
    } = Fixture::new();
    drop(messages);
    let replacement = manager
        .recreate_lxapp(old.appid.clone(), Channel::Release, old.session_id())
        .await
        .unwrap();
    assert_replaced(&manager, &old, &replacement);
}

#[tokio::test]
async fn open_during_the_handoff_gets_a_fresh_session_that_restart_adopts() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    let ack = f.take_ack();
    let opened = f.ensure_replacement();
    assert!(futures::poll!(restart.as_mut()).is_pending());
    ack.send(()).unwrap();
    let replacement = time::timeout(Duration::from_secs(1), restart)
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&replacement, &opened));
    assert!(!opened.session.is_cancelled());
}

#[tokio::test]
async fn restart_does_not_hold_admission_while_waiting_for_the_worker() {
    let f = Fixture::new();
    let restart =
        f.manager
            .recreate_lxapp(f.old.appid.clone(), Channel::Release, f.old.session_id());
    tokio::pin!(restart);
    assert!(futures::poll!(restart.as_mut()).is_pending());
    let _ack = f.take_ack();
    assert_eq!(*f.manager.admission.active.borrow(), 0);
}

#[tokio::test]
async fn restart_survives_the_old_session_being_destroyed_during_the_wait() {
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
        // LRU eviction can pick the retired session while it is still published.
        f.manager
            .with_session_transition(&f.old.appid, || f.manager.destroy_lxapp(&f.old.appid));
        if acknowledge {
            ack.send(()).unwrap();
        } else {
            drop(ack);
        }
        let replacement = time::timeout(Duration::from_secs(1), restart)
            .await
            .unwrap()
            .unwrap();
        assert_replaced(&f.manager, &f.old, &replacement);
        assert!(!replacement.session.is_cancelled());
    }
}
