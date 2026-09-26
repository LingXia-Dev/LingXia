use super::*;
use rong::{Rong, RongJS};
use serde_json::json;

/// Evaluate `script` in a fresh context with real timers and the dormant
/// controller, as a Logic context of an automation host has them. The
/// script sees `clock` (the controller) and `lease(token)` /
/// `unlease(token)`, which grant and drop the run lease of an install.
/// Resolves the script's JSON result; fails if it has not settled within
/// `EVAL_LIMIT`, so a stalled clock fails the test instead of hanging it.
fn eval_clock(script: &str) -> Value {
    const EVAL_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);
    let script = format!(
        "(async () => {{ const clock = {CONTROLLER}; {script} }})().then((value) => JSON.stringify(value))"
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let json = rt.block_on(async move {
        let pool = Rong::<RongJS>::builder()
            .shared()
            .workers(1)
            .build()
            .unwrap();
        let worker = pool.worker(0).unwrap();
        let handle = worker
            .spawn(async move |js_runtime, _receiver| -> JSResult<String> {
                let ctx = js_runtime.context();
                rong_modules::init(&ctx, ["timer"])?;
                let lease = JSFunc::new(&ctx, |token: f64| {
                    with_leases(|leases| {
                        leases.insert(
                            token as u64,
                            Lease {
                                run_id: "clock-test-run".into(),
                                appid: "clock-test-app".into(),
                            },
                        )
                    });
                })?;
                ctx.global().set("lease", lease)?;
                let unlease = JSFunc::new(&ctx, |token: f64| {
                    with_leases(|leases| leases.remove(&(token as u64)));
                })?;
                ctx.global().set("unlease", unlease)?;
                install_logic_clock(&ctx)?;
                let eval = ctx.eval_async::<String>(Source::from_bytes(script));
                tokio::time::timeout(EVAL_LIMIT, eval)
                    .await
                    .unwrap_or_else(|_| {
                        Err(auto_err(format!(
                            "the clock script did not settle within {EVAL_LIMIT:?}"
                        )))
                    })
            })
            .await
            .unwrap();
        handle.join().await.unwrap()
    });
    serde_json::from_str(&json).unwrap()
}

/// A token no other test uses: tests share the process lease table.
fn token() -> u64 {
    next_token() + 1_000_000
}

#[test]
fn timers_fire_in_time_order_and_only_when_due() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const log = [];
        clock.install({t}, 1000);
        setTimeout(() => log.push(['c', Date.now()]), 30);
        setTimeout(() => log.push(['a', Date.now()]), 10);
        setTimeout(() => log.push(['b1', Date.now()]), 20);
        setTimeout(() => log.push(['b2', Date.now()]), 20);
        const first = await clock.tick({t}, 25);
        const early = log.slice();
        const second = await clock.tick({t}, 5);
        clock.uninstall();
        return {{ early, log, first, second }};
        "#
    ));
    assert_eq!(
        result["early"],
        json!([["a", 1010], ["b1", 1020], ["b2", 1020]])
    );
    assert_eq!(result["log"][3], json!(["c", 1030]));
    assert_eq!(
        result["first"],
        json!({ "ok": true, "now": 1025, "fired": 3, "pending": 1 })
    );
    assert_eq!(
        result["second"],
        json!({ "ok": true, "now": 1030, "fired": 1, "pending": 0 })
    );
}

#[test]
fn nested_timers_and_intervals_fire_within_the_window() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const log = [];
        clock.install({t}, 0);
        setTimeout(() => {{
          log.push('outer@' + Date.now());
          setTimeout(() => log.push('zero@' + Date.now()), 0);
          setTimeout(() => log.push('later@' + Date.now()), 50);
        }}, 10);
        let beats = 0;
        const beat = setInterval(() => {{
          beats += 1;
          log.push('beat@' + Date.now());
          if (beats === 3) clearInterval(beat);
        }}, 3000);
        const ticked = await clock.tick({t}, 20);
        const afterTwenty = log.slice();
        const long = await clock.tick({t}, 20000);
        clock.uninstall();
        return {{ afterTwenty, log, ticked, long }};
        "#
    ));
    assert_eq!(result["afterTwenty"], json!(["outer@10", "zero@10"]));
    assert_eq!(
        result["log"],
        json!([
            "outer@10",
            "zero@10",
            "later@60",
            "beat@3000",
            "beat@6000",
            "beat@9000"
        ])
    );
    assert_eq!(result["long"]["pending"], 0);
}

#[test]
fn async_chains_started_by_a_timer_settle_before_the_next_timer() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const polls = [];
        clock.install({t}, 0);
        // A polling loop: each round awaits several promise hops and a real
        // native timer before it re-arms.
        const poll = async () => {{
          await Promise.resolve();
          await new Promise((resolve) => resolve());
          await null;
          polls.push(Date.now());
          setTimeout(poll, 3000);
        }};
        setTimeout(poll, 3000);
        const ticked = await clock.tick({t}, 9000);
        clock.uninstall();
        return {{ polls, ticked }};
        "#
    ));
    assert_eq!(result["polls"], json!([3000, 6000, 9000]));
    assert_eq!(result["ticked"]["fired"], 3);
    assert_eq!(result["ticked"]["pending"], 1);
}

#[test]
fn uninstall_restores_the_natives_and_drops_pending_timers() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        if (typeof performance === 'undefined' || typeof performance.now !== 'function') {{
          globalThis.performance = {{ now: () => 7 }};
        }}
        const perfNow = performance.now;
        const natives = [setTimeout, clearTimeout, setInterval, clearInterval, Date];
        const realBefore = Date.now();
        clock.install({t}, 5);
        const faked = natives.map((native, i) =>
          [setTimeout, clearTimeout, setInterval, clearInterval, Date][i] !== native);
        setTimeout(() => {{}}, 100);
        setInterval(() => {{}}, 100);
        const removed = clock.uninstall();
        const restored = [setTimeout, clearTimeout, setInterval, clearInterval, Date]
          .every((value, i) => value === natives[i]) && performance.now === perfNow;
        const again = clock.uninstall();
        const tickAfter = await clock.tick({t}, 10);
        // Real timers work again.
        const fired = await new Promise((resolve) => setTimeout(() => resolve(true), 1));
        return {{ faked, removed, again, restored, tickAfter, fired, realNow: Date.now() >= realBefore }};
        "#
    ));
    assert_eq!(result["faked"], json!([true, true, true, true, true]));
    assert_eq!(
        result["removed"],
        json!({ "uninstalled": true, "dropped": 2 })
    );
    assert_eq!(
        result["again"],
        json!({ "uninstalled": false, "dropped": 0 })
    );
    assert_eq!(result["restored"], true);
    assert_eq!(result["tickAfter"], json!({ "error": "not_installed" }));
    assert_eq!(result["fired"], true);
    assert_eq!(result["realNow"], true);
}

#[test]
fn stale_wrappers_forward_arguments_and_ids_never_repeat() {
    let t = token();
    let u = token() + 1;
    let result = eval_clock(&format!(
        r#"
        lease({t});
        clock.install({t}, 0);
        // A wrapper someone kept from the first install.
        const staleSetTimeout = setTimeout;
        const first = setTimeout(() => {{}}, 10);
        clock.uninstall();
        const viaStale = await new Promise((resolve) =>
          staleSetTimeout((a, b) => resolve([a, b]), 1, 'x', 2));
        lease({u});
        clock.install({u}, 0);
        const second = setTimeout(() => {{}}, 10);
        clock.uninstall();
        return {{ viaStale, distinct: first !== second }};
        "#
    ));
    assert_eq!(result["viaStale"], json!(["x", 2]));
    assert_eq!(result["distinct"], true);
}

#[test]
fn date_reads_fake_time_and_set_system_time_fires_nothing() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const RealDate = Date;
        clock.install({t}, '2030-01-02T03:04:05.000Z');
        const fired = [];
        setTimeout(() => fired.push(Date.now()), 1000);
        const noArgs = new Date().toISOString();
        const explicit = new Date(0).toISOString();
        const parts = new Date(2020, 0, 1).getFullYear();
        const called = typeof Date();
        const same = new Date() instanceof RealDate && new RealDate() instanceof Date;
        const set = clock.setSystemTime({t}, 1e12);
        const afterSet = Date.now();
        const ticked = await clock.tick({t}, 1000);
        const bad = clock.setSystemTime({t}, 'not a date');
        clock.uninstall();
        return {{ noArgs, explicit, parts, called, same, set, afterSet, fired, ticked, bad }};
        "#
    ));
    assert_eq!(result["noArgs"], "2030-01-02T03:04:05.000Z");
    assert_eq!(result["explicit"], "1970-01-01T00:00:00.000Z");
    assert_eq!(result["parts"], 2020);
    assert_eq!(result["called"], "string");
    assert_eq!(result["same"], true);
    assert_eq!(result["set"]["pending"], 1, "setSystemTime fires nothing");
    assert_eq!(result["afterSet"], 1e12);
    // The timer keeps its due time on the monotonic clock.
    assert_eq!(result["fired"], json!([1_000_000_001_000u64]));
    assert_eq!(result["bad"], json!({ "error": "invalid_time" }));
}

#[test]
fn installing_twice_is_refused_and_calls_need_the_owner_token() {
    let t = token();
    let other = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        lease({other});
        const first = clock.install({t}, 0);
        const same = clock.install({t}, 0);
        const second = clock.install({other}, 0);
        const foreign = await clock.tick({other}, 10);
        clock.uninstall();
        const bad = clock.install({t}, 'nonsense');
        return {{ first, same, second, foreign, bad }};
        "#
    ));
    assert_eq!(
        result["first"],
        json!({ "ok": true, "now": 0, "pending": 0 })
    );
    assert_eq!(result["same"], json!({ "error": "installed" }));
    assert_eq!(result["second"], json!({ "error": "installed" }));
    assert_eq!(result["foreign"], json!({ "error": "not_installed" }));
    assert_eq!(result["bad"], json!({ "error": "invalid_time" }));
}

#[test]
fn a_clock_whose_lease_ends_uninstalls_itself() {
    let t = token();
    let next = token();
    let result = eval_clock(&format!(
        r#"
        const RealDate = Date;
        const realSetTimeout = setTimeout;
        lease({t});
        clock.install({t}, 0);
        // A stale clock does not block the next run's install.
        unlease({t});
        lease({next});
        const replaced = clock.install({next}, 0);
        // The run ends: the watchdog notices on its real one-second interval.
        unlease({next});
        const stillFake = Date !== RealDate;
        await new Promise((resolve) => realSetTimeout(resolve, 1500));
        return {{ replaced, stillFake, restored: Date === RealDate, state: clock.state({next}) }};
        "#
    ));
    assert_eq!(
        result["replaced"],
        json!({ "ok": true, "now": 0, "pending": 0 })
    );
    assert_eq!(result["stillFake"], true);
    assert_eq!(result["restored"], true);
    assert_eq!(
        result["state"],
        json!({ "installed": false, "owned": false })
    );
}

#[test]
fn timers_from_before_install_stay_real_and_errors_do_not_stop_the_tick() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const log = [];
        let realFired = false;
        const realId = setTimeout(() => {{ realFired = true; }}, 30);
        setTimeout(() => log.push('real'), 20);
        clock.install({t}, 0);
        clearTimeout(realId);
        let beats = 0;
        setInterval(() => {{ beats += 1; throw new Error('boom'); }}, 10);
        setTimeout(() => log.push('after-error'), 25);
        const ticked = await clock.tick({t}, 100);
        // Let the real timer that was kept fire on real time.
        clock.uninstall();
        await new Promise((resolve) => setTimeout(resolve, 60));
        return {{ log, beats, realFired, ticked }};
        "#
    ));
    assert_eq!(result["realFired"], false, "a real timer is still cleared");
    assert_eq!(result["beats"], 1, "a throwing interval is cancelled");
    assert!(
        result["log"]
            .as_array()
            .unwrap()
            .contains(&json!("after-error"))
    );
    assert!(
        result["log"].as_array().unwrap().contains(&json!("real")),
        "a timer started before install fires on real time"
    );
    assert_eq!(result["ticked"]["pending"], 0);
}

#[test]
fn run_all_drains_timers_and_refuses_an_endless_interval() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const log = [];
        clock.install({t}, 0);
        setTimeout(() => {{ log.push(Date.now()); setTimeout(() => log.push(Date.now()), 500); }}, 1000);
        const drained = await clock.runAll({t}, 1000);
        setInterval(() => {{}}, 10);
        const endless = await clock.runAll({t}, 50);
        clock.uninstall();
        return {{ log, drained, endless }};
        "#
    ));
    assert_eq!(result["log"], json!([1000, 1500]));
    assert_eq!(
        result["drained"],
        json!({ "ok": true, "now": 1500, "fired": 2, "pending": 0 })
    );
    assert_eq!(result["endless"]["error"], "limit");
    assert_eq!(result["endless"]["fired"], 50);
}

#[test]
fn settling_survives_thousands_of_turns() {
    // Each firing settles on at least three real turns. With real zero-delay
    // timers for turns, one of these thousands was eventually dropped by the
    // runtime and the run never finished.
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        clock.install({t}, 0);
        let beats = 0;
        setInterval(() => {{ beats += 1; }}, 10);
        const ran = await clock.runAll({t}, 2000);
        clock.uninstall();
        return {{ ran, beats }};
        "#
    ));
    assert_eq!(result["ran"]["error"], "limit");
    assert_eq!(result["ran"]["fired"], 2000);
    assert_eq!(result["beats"], 2000);
}

#[test]
fn performance_now_follows_the_fake_clock_when_present() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        if (typeof performance === 'undefined' || typeof performance.now !== 'function') {{
          globalThis.performance = {{ now: () => 7 }};
        }}
        const native = performance.now;
        clock.install({t}, 0);
        const start = performance.now();
        await clock.tick({t}, 250);
        const elapsed = performance.now() - start;
        clock.uninstall();
        return {{ elapsed, restored: performance.now === native }};
        "#
    ));
    assert_eq!(result["elapsed"], 250);
    assert_eq!(result["restored"], true);
}

#[test]
fn argument_checks() {
    assert!(tick_ms(0.0).is_ok());
    assert!(tick_ms(1.5).is_ok());
    assert!(tick_ms(-1.0).is_err());
    assert!(tick_ms(f64::NAN).is_err());
    assert!(tick_ms(f64::INFINITY).is_err());
    assert!(tick_ms(MAX_TICK_MS + 1.0).is_err());
    assert_eq!(run_all_limit(None), Ok(DEFAULT_RUN_ALL_TIMERS));
    assert_eq!(run_all_limit(Some(5.0)), Ok(5));
    for bad in [0.0, 1.5, -1.0, f64::from(MAX_RUN_ALL_TIMERS) + 1.0] {
        assert!(run_all_limit(Some(bad)).is_err(), "{bad}");
    }
    assert_eq!(
        outcome(json!({ "error": "limit", "fired": 3, "pending": 1 })),
        Outcome::Limit {
            fired: 3,
            pending: 1
        }
    );
    assert_eq!(
        outcome(json!({ "error": "not_installed" })),
        Outcome::NotInstalled
    );
    assert!(matches!(outcome(json!({ "ok": true })), Outcome::Ok(_)));
}

#[test]
fn leases_are_scoped_to_their_run_and_app() {
    let insert = |token: u64, run: &str, app: &str| {
        with_leases(|leases| {
            leases.insert(
                token,
                Lease {
                    run_id: run.into(),
                    appid: app.into(),
                },
            )
        });
    };
    let (a, b, c) = (token(), token(), token());
    insert(a, "lease-run-1", "app.one");
    insert(b, "lease-run-1", "app.one");
    insert(c, "lease-run-1", "app.two");
    assert_eq!(lease_for("lease-run-1", "app.one"), Some(b));
    release_app("lease-run-1", "app.one", Some(b));
    assert!(!is_leased(a) && is_leased(b) && is_leased(c));
    clear_run("lease-run-1");
    assert!(!is_leased(b) && !is_leased(c));
    assert_eq!(lease_for("lease-run-1", "app.one"), None);
}

/// `install()`, `install(undefined)` and `install(null)` all mean "no
/// options"; so do the same forms of `runAll`.
#[test]
fn options_arguments_accept_undefined_and_null() {
    use rong::{JSEngine, function::Optional};
    let runtime = RongJS::runtime();
    let ctx = runtime.context();
    let install = JSFunc::new(
        &ctx,
        |options: Optional<Option<InstallOptions>>| -> String {
            match options.0.flatten() {
                None => "none".into(),
                Some(options) => format!("now:{}", options.now.is_some()),
            }
        },
    )
    .unwrap();
    ctx.global().set("install", install).unwrap();
    let run_all = JSFunc::new(&ctx, |options: Optional<Option<RunAllOptions>>| -> String {
        match options.0.flatten() {
            None => "none".into(),
            Some(options) => format!("max:{:?}", options.max_timers),
        }
    })
    .unwrap();
    ctx.global().set("runAll", run_all).unwrap();
    let out: String = ctx
        .eval(Source::from_bytes(
            "JSON.stringify([install(), install(undefined), install(null), install({ now: 5 }), \
             runAll(), runAll(undefined), runAll(null), runAll({ maxTimers: 3 })])",
        ))
        .unwrap();
    assert_eq!(
        out,
        r#"["none","none","none","now:true","none","none","none","max:Some(3.0)"]"#
    );
}

#[test]
fn install_and_set_system_time_resolve_the_clock_state() {
    let t = token();
    let result = eval_clock(&format!(
        r#"
        lease({t});
        const installed = clock.install({t}, 5);
        setTimeout(() => {{}}, 10);
        const set = clock.setSystemTime({t}, 50);
        clock.uninstall();
        return {{ installed, set }};
        "#
    ));
    assert_eq!(
        super::state(&result["installed"]),
        super::ClockState {
            now: 5.0,
            pending: 0.0
        }
    );
    assert_eq!(
        super::state(&result["set"]),
        super::ClockState {
            now: 50.0,
            pending: 1.0
        }
    );
}
