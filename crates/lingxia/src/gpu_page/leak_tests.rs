use super::*;

/// Pages created and released many times leave nothing behind in the registry, the pack
/// holders or the theme store, and each shell is disposed exactly once.
#[test]
fn a_thousand_page_lifetimes_leave_no_registry_or_pack_state() {
    const CYCLES: usize = 1000;
    let factory = Arc::new(Factory::default());
    let shared: Arc<dyn NativeFactory> = factory.clone();
    let pack = || bundled_pack(r##"{"name":"leak","version":1}"##);
    let mut tokens = Vec::with_capacity(CYCLES);
    let mut workers = std::collections::HashSet::new();
    for cycle in 0..CYCLES {
        // A relaunch overlaps two pages of one session; sessions come and go.
        let worker = format!("leak:{}", cycle / 3);
        let token = NEXT.fetch_add(1, Ordering::Relaxed);
        let page = format!("leak-page-{}", cycle % 7);
        let lease = Lease::new(token, page, worker.clone(), pack(), shared.clone());
        match cycle % 3 {
            // Created, shown, disposed by lxapp, then dropped.
            0 => {
                let endpoint = register(lease, Arc::new(Shell::default()));
                endpoint
                    .connect(GpuViewGeneration::new_for_test(1))
                    .unwrap();
                endpoint.dispose();
                drop(endpoint);
            }
            // Dropped without an explicit dispose.
            1 => drop(register(lease, Arc::new(Shell::default()))),
            // Creation abandoned while still pending.
            _ => {
                let (tx, _rx) = oneshot::channel();
                registry().pending.insert(token, tx);
                drop(lease);
                created(token, Err("late".into()));
            }
        }
        tokens.push(token);
        workers.insert(worker);
    }

    let registry = registry();
    for token in &tokens {
        assert!(!registry.pending.contains_key(token), "pending {token}");
        assert!(!registry.live.contains_key(token), "live {token}");
        assert!(
            !registry.pages.values().any(|live| live == token),
            "page {token}"
        );
    }
    drop(registry);
    let holders = PACK_HOLDERS.lock().unwrap();
    for worker in &workers {
        assert!(!holders.contains_key(worker), "holder {worker}");
        assert!(lingxia_theme::app_pack(worker).is_none(), "pack {worker}");
    }
    drop(holders);
    let mut disposed = factory.0.lock().unwrap().clone();
    disposed.retain(|token| tokens.contains(token));
    disposed.sort_unstable();
    assert_eq!(disposed, tokens, "every shell disposed exactly once");
}
