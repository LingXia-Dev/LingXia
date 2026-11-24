use super::*;
use crate::{LxAppDelegate, LxAppUiEventResult, LxAppUiEventType};

const HOME: &str = "pages/home/index";
const OTHER: &str = "pages/me/index";
const DETAIL: &str = "pages/detail/index";

struct Fixture {
    app: Arc<LxApp>,
    _root: tempfile::TempDir,
    #[cfg(target_vendor = "apple")]
    _host: Box<dyn std::any::Any>,
}

impl Fixture {
    fn new(home: bool) -> Self {
        #[cfg(target_vendor = "apple")]
        let host = Box::new(crate::apple_host_stubs::headless_lifecycle());
        let root = tempfile::tempdir().unwrap();
        let runtime = Arc::new(
            Platform::new(
                root.path().join("data").display().to_string(),
                root.path().join("cache").display().to_string(),
                "en-US".into(),
            )
            .unwrap(),
        );
        let mut app = LxApp::_new(
            format!("app.lingxia.system-back.{}", Uuid::new_v4()),
            runtime,
            LxAppWorkers::init(1),
            Channel::Release,
            if home {
                AppSessionClass::ControlApp
            } else {
                AppSessionClass::StandardApp
            },
        );
        app.is_home_lxapp = home;
        let config = LxAppConfig::from_value(serde_json::json!({
            "version": "1.0.0",
            "logic": false,
            "pages": [
                { "name": "home", "path": HOME },
                { "name": "me", "path": OTHER },
                { "name": "detail", "path": DETAIL }
            ],
            "tabBar": { "items": [{ "page": "home" }, { "page": "me" }] }
        }))
        .unwrap();
        app.state.lock().unwrap().tabbar = config.tabBar.clone();
        *app.config.lock().unwrap() = config;
        let app = Arc::new(app);
        app.bind_arc();
        app.set_status(LxAppSessionStatus::Opened);
        for path in [HOME, OTHER, DETAIL] {
            let page = PageInstance::new_headless(app.appid.clone(), path.into(), &app);
            app.state
                .lock()
                .unwrap()
                .pages_by_id
                .lock()
                .unwrap()
                .insert(page.instance_id_string(), page.clone());
            app.pin_page_path(&page);
        }
        Self {
            app,
            _root: root,
            #[cfg(target_vendor = "apple")]
            _host: host,
        }
    }

    fn push(&self, path: &str) -> PageInstance {
        let page = self.app.get_page(path).unwrap();
        self.app.push_to_page_stack(&page).unwrap();
        page
    }

    fn back(&self) -> LxAppUiEventResult {
        self.app
            .on_lxapp_event_result(LxAppUiEventType::BackPress, String::new())
    }
}

#[test]
fn home_back_requests_background_without_closing_or_resetting_the_session() {
    let f = Fixture::new(true);
    let page = f.push(HOME);
    let session = f.app.session_id();
    assert_eq!(f.back(), LxAppUiEventResult::BackgroundHost);
    assert_eq!(f.app.session_id(), session);
    assert_eq!(f.app.status(), LxAppSessionStatus::Opened);
    assert!(!f.app.session.is_cancelled());
    assert_eq!(
        f.app.current_page().unwrap().instance_id(),
        page.instance_id()
    );
    assert_eq!(f.app.get_page_stack_size(), 1);
}

#[test]
fn details_pop_before_home_back_can_background_the_host() {
    let f = Fixture::new(true);
    f.push(OTHER);
    f.push(DETAIL);
    assert_eq!(f.back(), LxAppUiEventResult::Handled);
    assert_eq!(f.app.peek_current_page_path().as_deref(), Some(OTHER));
    assert_eq!(f.app.get_page_stack_size(), 1);
    assert_eq!(f.back(), LxAppUiEventResult::Handled);
    assert_eq!(f.app.peek_current_page_path().as_deref(), Some(HOME));
    assert_eq!(f.app.get_page_stack_size(), 1);
    assert_eq!(f.back(), LxAppUiEventResult::BackgroundHost);
}

#[test]
fn a_guarded_detail_stays_in_front_instead_of_backgrounding() {
    let f = Fixture::new(true);
    f.push(HOME);
    let detail = f.push(DETAIL);
    detail.set_leave_guard(true);
    assert_eq!(f.back(), LxAppUiEventResult::Handled);
    assert_eq!(f.app.get_page_stack_size(), 2);
    assert_eq!(
        f.app.current_page().unwrap().instance_id(),
        detail.instance_id()
    );
}

#[test]
fn another_home_tab_returns_home_on_the_first_back() {
    let f = Fixture::new(true);
    f.push(OTHER);
    f.app.with_tabbar_mut(|tabbar| tabbar.set_selected_index(1));
    assert_eq!(f.back(), LxAppUiEventResult::Handled);
    assert_eq!(f.app.peek_current_page_path().as_deref(), Some(HOME));
    assert_eq!(f.app.get_tabbar().unwrap().get_selected_index(), 0);
    assert_eq!(f.app.get_page_stack_size(), 1);
    assert_eq!(f.back(), LxAppUiEventResult::BackgroundHost);
}

#[test]
fn a_guest_root_never_requests_host_background() {
    let f = Fixture::new(false);
    f.push(HOME);
    assert_eq!(f.back(), LxAppUiEventResult::Handled);
}

#[test]
fn an_empty_or_unresolved_home_stack_never_requests_host_background() {
    let f = Fixture::new(true);
    assert_eq!(f.back(), LxAppUiEventResult::Unhandled);
    let page = f.push(HOME);
    f.app
        .state
        .lock()
        .unwrap()
        .pages_by_id
        .lock()
        .unwrap()
        .remove(&page.instance_id_string());
    assert_eq!(f.back(), LxAppUiEventResult::Unhandled);
}

#[test]
fn the_navbar_back_button_and_boolean_delegates_do_not_exit_the_host() {
    let f = Fixture::new(true);
    f.push(HOME);
    assert_eq!(
        f.app
            .on_lxapp_event_result(LxAppUiEventType::NavigationClick, "back".into()),
        LxAppUiEventResult::Handled,
    );
    assert!(
        f.app
            .on_lxapp_event(LxAppUiEventType::BackPress, String::new())
    );
    assert_eq!(f.app.get_page_stack_size(), 1);
}
