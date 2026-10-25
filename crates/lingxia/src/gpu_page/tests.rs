use super::*;

#[derive(Default)]
struct Shell(Mutex<Vec<String>>);

impl Shell {
    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl NativePage for Shell {
    fn connect(&self, generation: GpuViewGeneration, nonce: &str) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("connect {} {nonce}", generation.get()));
        Ok(())
    }
    fn receive(&self, generation: GpuViewGeneration, frame: &str) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("receive {} {frame}", generation.get()));
        Ok(())
    }
    fn request_leave(&self, generation: GpuViewGeneration, reason: &str) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("leave {} {reason}", generation.get()));
        Ok(())
    }
    fn update_environment(&self, update: &serde_json::Value) -> Result<(), LxAppError> {
        self.0
            .lock()
            .unwrap()
            .push(format!("environment {}", update["kind"]));
        Ok(())
    }
    fn reload(&self, _: &str) -> Result<(), LxAppError> {
        Ok(())
    }
    fn park(&self) -> Result<(), LxAppError> {
        self.0.lock().unwrap().push("park".into());
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct Factory(Mutex<Vec<Token>>);

impl NativeFactory for Factory {
    fn create(&self, _: Token, _: &PageInstance, _: Creation) -> Result<(), LxAppError> {
        unreachable!("tests register endpoints directly")
    }
    fn dispose(&self, token: Token) {
        self.0.lock().unwrap().push(token);
    }
}

fn endpoint(factory: &Arc<Factory>, page: &str) -> (Arc<Endpoint>, Arc<Shell>, Token) {
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let shell = Arc::new(Shell::default());
    let lease = Lease {
        token,
        page: page.into(),
        disposed: AtomicBool::new(false),
        factory: factory.clone(),
    };
    (register(lease, shell.clone()), shell, token)
}

fn generation(value: u64) -> GpuViewGeneration {
    GpuViewGeneration::new_for_test(value)
}

#[test]
fn messages_reach_the_shell_only_for_the_current_connection() {
    let factory = Arc::new(Factory::default());
    let (endpoint, shell, _) = endpoint(&factory, "gate");
    endpoint
        .post_message_for_generation(generation(1), "early")
        .unwrap();
    endpoint.connect(generation(2), "nonce").unwrap();
    endpoint
        .post_message_for_generation(generation(1), "stale")
        .unwrap();
    endpoint
        .post_message_for_generation(generation(2), "state")
        .unwrap();
    assert!(
        endpoint.connect(generation(1), "old").is_err(),
        "superseded"
    );
    endpoint.request_leave(LeaveReason::Back).unwrap();
    endpoint.park().unwrap();
    endpoint.request_leave(LeaveReason::Home).unwrap();
    endpoint
        .post_message_for_generation(generation(2), "parked")
        .unwrap();
    assert_eq!(
        shell.calls(),
        ["connect 2 nonce", "receive 2 state", "leave 2 back", "park"]
    );
}

#[test]
fn a_lease_disposes_once_and_leaves_no_index_behind() {
    let factory = Arc::new(Factory::default());
    let (endpoint, shell, token) = endpoint(&factory, "dispose-once");
    assert!(live(token).is_some());
    endpoint.connect(generation(3), "n").unwrap();
    endpoint.dispose();
    endpoint.dispose();
    endpoint
        .post_message_for_generation(generation(3), "late")
        .unwrap();
    endpoint
        .update_environment(PageViewEnvironment::DisplayLanguage {
            language: "en".into(),
            revision: 1,
        })
        .unwrap();
    assert!(live(token).is_none());
    assert!(!registry().pages.contains_key("dispose-once"));
    drop(endpoint);
    assert_eq!(*factory.0.lock().unwrap(), [token]);
    assert_eq!(shell.calls(), ["connect 3 n"]);
}

#[test]
fn a_replaced_shell_keeps_the_page_index_when_the_old_one_retires() {
    let factory = Arc::new(Factory::default());
    let (old, _, _) = endpoint(&factory, "replaced");
    let (_new, _, token) = endpoint(&factory, "replaced");
    old.dispose();
    assert_eq!(registry().pages.get("replaced"), Some(&token));
}

#[test]
fn creation_outcomes_reach_only_their_pending_request() {
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = oneshot::channel();
    registry().pending.insert(token, tx);
    created(token + 1_000_000, Err("other".into()));
    assert!(rx.try_recv().is_err());
    created(token, Err("failed".into()));
    assert_eq!(rx.try_recv().unwrap().err().as_deref(), Some("failed"));
    assert_eq!(worker_key("app", 7), "app:7");
}
