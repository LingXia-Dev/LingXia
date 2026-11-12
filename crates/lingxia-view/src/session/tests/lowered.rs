//! Between frames, a page's lowered styles are offered to the cache once, without running
//! any of the page's pending jobs.
use crate::ViewSession;
use rong::Source;

fn page() -> ViewSession {
    let module =
        super::source("function render(){return h('page',{},h('view',{},h('text',{},'a')))}");
    let session = ViewSession::new(&module).unwrap();
    // Count takes; the host globals are plain properties.
    eval(
        &session,
        "globalThis.__takes=0;const take=__lingxiaTakeLowered;globalThis.__lingxiaTakeLowered=()=>(__takes++,take())",
    );
    session
}

fn eval(session: &ViewSession, source: &str) -> i32 {
    session
        .context
        .eval::<i32>(Source::from_bytes(source))
        .unwrap_or(0)
}

#[test]
fn a_between_frames_take_runs_none_of_the_pages_jobs() {
    let mut session = page();
    session.layout(200, 100, 1.0).unwrap();
    eval(
        &session,
        "globalThis.__ran=0;Promise.resolve().then(()=>{globalThis.__ran=1});0",
    );
    for _ in 0..3 {
        session.collect_between_frames();
    }
    assert_eq!(eval(&session, "__takes"), 1);
    assert_eq!(
        eval(&session, "__ran"),
        0,
        "a pending job waits for the next frame"
    );
}

#[test]
fn styles_are_taken_once_per_isolate_however_many_frames_commit() {
    let mut session = page();
    for width in [200, 220, 240] {
        session.layout(width, 100, 1.0).unwrap();
        session.collect_between_frames();
    }
    assert_eq!(eval(&session, "__takes"), 1);
}

#[test]
fn nothing_is_taken_before_the_first_commit() {
    let session = page();
    session.collect_between_frames();
    assert_eq!(eval(&session, "__takes"), 0);
}
