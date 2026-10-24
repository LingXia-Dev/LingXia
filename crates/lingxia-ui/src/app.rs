use crate::ViewSession;
use rong::{JSEngine, RongJS};
use std::collections::HashMap;

/// Native-owned identity; never reuse an id after its context is discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewPageId(u64);

/// One isolate per app. Construct, access and drop on the app's View executor.
/// Contexts contain no host capabilities; these ids confer no route authority.
pub struct ViewApp {
    // Drop contexts before the runtime that owns them.
    pages: HashMap<ViewPageId, ViewSession>,
    runtime: rong::JSRuntime,
    budget: std::sync::Arc<crate::budget::Budget>,
    next: u64,
    limit: usize,
}

impl ViewApp {
    pub fn new(page_limit: usize) -> Result<Self, String> {
        if page_limit == 0 || page_limit > 64 {
            return Err("UI_LIMIT: page limit must be between 1 and 64".into());
        }
        let runtime = RongJS::runtime();
        let budget = crate::budget::Budget::new(&runtime);
        Ok(Self {
            pages: HashMap::new(),
            runtime,
            budget,
            next: 1,
            limit: page_limit,
        })
    }

    pub fn create(&mut self, source: &str, logic: bool) -> Result<ViewPageId, String> {
        if self.pages.len() >= self.limit {
            return Err("UI_LIMIT: discard a page before creating another context".into());
        }
        let next = self
            .next
            .checked_add(1)
            .ok_or("UI_LIMIT: page identities exhausted")?;
        let session =
            ViewSession::with_runtime(self.runtime.clone(), self.budget.clone(), source, logic)?;
        let id = ViewPageId(self.next);
        self.next = next;
        self.pages.insert(id, session);
        Ok(id)
    }

    pub fn page(&mut self, id: ViewPageId) -> Result<&mut ViewSession, String> {
        self.pages
            .get_mut(&id)
            .ok_or_else(|| "UI_PAGE: discarded context".into())
    }

    pub fn discard(&mut self, id: ViewPageId) -> bool {
        self.pages.remove(&id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::source;

    #[test]
    fn runaway_action_microtasks_are_drained_before_a_sibling_runs() {
        let mut app = ViewApp::new(2).unwrap();
        let bad = app.create(&source("function render(){return h('button',{onClick:()=>{const loop=()=>Promise.resolve().then(loop);loop()}},'run')}"), false).unwrap();
        let good = app
            .create(
                &source("function render(){return h('text',{},'healthy')}"),
                false,
            )
            .unwrap();
        let page = app.page(bad).unwrap();
        let frame = page.layout(300, 500, 1.0).unwrap();
        assert!(
            page.click(page.generation(), frame.nodes[0].on_click.unwrap())
                .unwrap_err()
                .starts_with("UI_TIMEOUT:")
        );
        app.discard(bad);
        assert_eq!(
            app.page(good).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
            "healthy"
        );
    }

    #[test]
    fn page_contexts_are_isolated_and_discarded_ids_cannot_target_replacements() {
        let mut app = ViewApp::new(2).unwrap();
        let source = source(
            "let moduleCount=0;function render(){const n=useLocal('n',0);return h('button',{onClick:()=>{n.set(n.value+1);moduleCount++}},`${n.value}/${moduleCount}`)}",
        );
        let a = app.create(&source, false).unwrap();
        let b = app.create(&source, false).unwrap();
        assert!(app.create(&source, false).is_err());
        let first = app.page(a).unwrap().layout(300, 500, 1.0).unwrap();
        app.page(a)
            .unwrap()
            .click(1, first.nodes[0].on_click.unwrap())
            .unwrap();
        assert_eq!(
            app.page(a).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
            "1/1"
        );
        assert_eq!(
            app.page(b).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
            "0/0"
        );
        app.page(a).unwrap().reset().unwrap();
        assert!(
            !app.page(a)
                .unwrap()
                .click(1, first.nodes[0].on_click.unwrap())
                .unwrap()
        );
        assert_eq!(
            app.page(a).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
            "0/1"
        );
        assert!(app.discard(a));
        assert!(!app.discard(a));
        let c = app.create(&source, false).unwrap();
        assert_ne!(a, c);
        assert!(app.page(a).is_err());
        assert_eq!(
            app.page(c).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
            "0/0"
        );
    }

    #[test]
    fn failed_module_does_not_consume_a_page_slot() {
        let mut app = ViewApp::new(1).unwrap();
        assert!(app.create("throw Error('bad module')", false).is_err());
        assert!(
            app.create(
                &source("function render(){return h('text',{},'ok')}"),
                false
            )
            .is_ok()
        );
    }

    #[test]
    #[cfg(any(
        target_os = "android",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    fn infinite_guest_code_is_interrupted_without_poisoning_other_pages() {
        let mut app = ViewApp::new(3).unwrap();
        let started = std::time::Instant::now();
        assert!(
            app.create("while(true){}", false)
                .unwrap_err()
                .contains("UI_TIMEOUT")
        );
        let bad = app
            .create(&source("function render(){while(true){}}"), false)
            .unwrap();
        assert!(
            app.page(bad)
                .unwrap()
                .layout(300, 500, 1.0)
                .err()
                .unwrap()
                .contains("UI_TIMEOUT")
        );
        assert_eq!(app.page(bad).unwrap().generation(), 0);
        assert!(
            app.page(bad)
                .unwrap()
                .reset()
                .unwrap_err()
                .contains("UI_TIMEOUT")
        );
        let click = app
            .create(
                &source(
                    "function render(){return h('button',{onClick:()=>{while(true){}}},'hang')}",
                ),
                false,
            )
            .unwrap();
        let frame = app.page(click).unwrap().layout(300, 500, 1.0).unwrap();
        assert!(
            app.page(click)
                .unwrap()
                .click(1, frame.nodes[0].on_click.unwrap())
                .unwrap_err()
                .contains("UI_TIMEOUT")
        );
        let good = app
            .create(
                &source("function render(){return h('text',{},'still alive')}"),
                false,
            )
            .unwrap();
        for _ in 0..20 {
            assert_eq!(
                app.page(good).unwrap().layout(300, 500, 1.0).unwrap().nodes[0].text,
                "still alive"
            );
        }
        assert!(app.discard(bad));
        assert!(app.discard(click));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}
