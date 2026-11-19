//! The Windows adapter for the GPU View host: one [`Page`] per content window. A page lives on
//! the thread that made it, which must pump messages; every [`Page`] method runs there.
#![cfg(windows)]
use lingxia_view::platform::DeviceFacts;
use lingxia_view::renderer::render_thread as render;
use lingxia_view::session::host::{Failure, ViewHost, ViewWorker};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CreateRectRgn, DeleteObject, GetMonitorInfoW, HRGN, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, SetWindowRgn,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GA_ROOT, GetAncestor, GetParent, HWND_TOP, IsWindowVisible, SW_HIDE,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetParent,
    SetWindowPos, ShowWindow,
};

mod adapter;
mod clipboard;
mod composition;
mod dispatch;
mod environment;
mod fonts;
mod ime;
mod recovery;
mod surface;
mod text_menu;
mod touch;
mod uia;
mod video;
mod view;
mod vsync;
mod web_input;
pub use web_input::adopt_web_surfaces;

pub use adapter::WindowsAdapter;
pub use dispatch::Dispatcher;
pub use lingxia_view;

/// The embedding host's half: the lxapp page binding and the chrome around the page.
pub trait Shell: Send + Sync + 'static {
    /// A frame for `connection` reached the screen; 0 is a standalone page.
    fn presented(&self, token: i64, connection: u64);
    /// The View worker thread: what the View sends Logic for `connection`, in order.
    fn logic(
        &self,
        token: i64,
        connection: u64,
        messages: Vec<lingxia_view::session::logic::ToLogic>,
    );
    /// What became of Logic's typed state writes for `connection`.
    fn state(&self, token: i64, connection: u64, signal: lingxia_view::session::state::StateSignal);
    /// The View worker thread: a line the page's `console` wrote.
    fn console(&self, token: i64, level: log::Level, message: &str) {
        let _ = token;
        log::log!(target: "lingxia_view::console", level, "{message}");
    }
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, token: i64, connection: u64) -> bool;
    fn recovery(&self, token: i64, failure: Option<&Failure>);
    /// Host chrome text for a localizer `key`, else `fallback`.
    fn text(&self, key: &str, fallback: &str) -> String {
        let _ = key;
        fallback.to_owned()
    }
    fn bootstrap_overdue(&self, token: i64, overdue: bool);
    /// The colours the host paints around this page while it is shown.
    fn host_chrome(&self, token: i64, chrome: &lingxia_view::platform::HostChrome);
    /// A top-layer dialog opened or closed.
    fn top_layer(&self, token: i64, modal: bool);
    /// Whether the host's navigation bar and tab bar show over this page on a simulated device.
    fn bars(&self, token: i64) -> [bool; 2] {
        let _ = token;
        [false, false]
    }
    /// Load authorized image bytes for the page; `done` runs once unless the
    /// returned handle is dropped first, which cancels the load.
    fn load_image(
        &self,
        token: i64,
        source: String,
        done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
    ) -> Box<dyn Send>;
    /// The URL the player opens for the page's `<video src>`.
    fn video_url(&self, token: i64, source: &str) -> Result<String, String>;
    /// Reconcile the page's embedded `<webview>` mounts; on the page's thread.
    fn sync_webviews(&self, token: i64, mounts: &[lingxia_view::scene::webview::WebViewMount]);
    /// Capture each web view for `ViewHost::webview_picture`.
    fn picture_webviews(&self, token: i64, web_ids: &[String], scale: f32);
    fn exit_webview_fullscreen(&self, token: i64, web_id: &str);
    /// Tear down every embedded `<webview>` of the page.
    fn clear_webviews(&self, token: i64);
}

/// Pump this thread's messages until [`quit_message_loop`] runs on it.
pub fn run_message_loop() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };
    let mut msg = MSG::default();
    // SAFETY: the standard loop over this thread's queue; GetMessageW is 0 on WM_QUIT, -1 on
    // error.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// End [`run_message_loop`] on the calling thread once the current message returns.
pub fn quit_message_loop() {
    // SAFETY: posts WM_QUIT to this thread.
    unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
}

/// The colour scheme the page's lxapp asks for; `Auto` follows the Windows app theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Auto,
    Light,
    Dark,
}

/// A simulated device (the Runner): what its pages measure, whatever size the window shows
/// them at. Lengths are the device's points.
#[derive(Clone, Debug, PartialEq)]
pub struct Simulation {
    /// The screen in its present orientation.
    pub viewport: [f32; 2],
    pub pixel_ratio: f32,
    /// System bars and cutouts: top, right, bottom, left.
    pub safe_area: [f32; 4],
    /// The host's navigation bar, below the safe area's top.
    pub navigation_bar: f32,
    /// The host's tab bar, from the screen's bottom edge to its top.
    pub tab_bar_reach: f32,
    pub smallest_width: f32,
    /// System text size as a factor of the default.
    pub text_scale: f32,
    pub high_contrast: bool,
}

static SIMULATION: Mutex<Option<Simulation>> = Mutex::new(None);
/// Every live page, for a change that reaches them all.
static PAGES: Mutex<Vec<(isize, Dispatcher)>> = Mutex::new(Vec::new());

fn simulation() -> Option<Simulation> {
    SIMULATION.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The simulated device's viewport in points, while one is simulated.
pub fn simulated_viewport() -> Option<[f32; 2]> {
    simulation().map(|s| s.viewport)
}

/// Simulate a device for every page, or (`None`) measure the window as a desktop page does.
pub fn set_simulation(simulation: Option<Simulation>) {
    let simulation = simulation.filter(|s| {
        s.viewport.iter().all(|v| v.is_finite() && *v > 0.0)
            && s.pixel_ratio.is_finite()
            && s.pixel_ratio > 0.0
    });
    {
        let mut current = SIMULATION.lock().unwrap_or_else(|p| p.into_inner());
        if *current == simulation {
            return;
        }
        *current = simulation;
    }
    let pages = PAGES.lock().unwrap_or_else(|p| p.into_inner()).clone();
    for (hwnd, dispatcher) in pages {
        dispatcher.post(Box::new(move || {
            if let Some(page) = view::page_for(hwnd) {
                page.report();
            }
        }));
    }
}

/// Pages coming on screen over the pages they replace, clipped until they present. Each page
/// has its own thread, so these are shared and a page hides itself on its own.
static SETTLING: Mutex<Vec<isize>> = Mutex::new(Vec::new());
/// Hidden pages left on screen until no page is settling, with their threads.
static LEAVING: Mutex<Vec<(isize, Dispatcher)>> = Mutex::new(Vec::new());

fn settling() -> MutexGuard<'static, Vec<isize>> {
    SETTLING.lock().unwrap_or_else(|p| p.into_inner())
}

fn leaving() -> MutexGuard<'static, Vec<(isize, Dispatcher)>> {
    LEAVING.lock().unwrap_or_else(|p| p.into_inner())
}

/// Hide the pages left on screen for a page that settled, each on its own thread.
fn release_left() {
    let left = std::mem::take(&mut *leaving());
    for (page, dispatcher) in left {
        dispatcher.post(Box::new(move || hide_left(page)));
    }
}

/// On `hwnd`'s thread: off screen unless shown again meanwhile.
fn hide_left(hwnd: isize) {
    if let Some(page) = view::page_for(hwnd)
        && !page.visible.get()
    {
        page.show_window(false);
    }
}

pub struct Page {
    host: ViewHost<WindowsAdapter>,
    hwnd: isize,
    pub(crate) input: RefCell<view::InputState>,
    /// Physical pixels per CSS pixel the host imposes (a fit-scaled simulated device).
    scale: Cell<Option<f32>>,
    scheme: Cell<Scheme>,
    /// A presenter is bound, or its attach is pending.
    attached: Cell<bool>,
    visible: Cell<bool>,
    /// Shown clipped to one pixel until a frame of its geometry reaches the screen.
    settling: Cell<bool>,
    /// Size and corner radii from the host, device pixels.
    shape: Cell<(i32, i32, [i32; 4])>,
    /// Bumped to revoke a pending settle deadline.
    settle_epoch: Cell<u64>,
    /// Frames presented while settling.
    settle_presents: Cell<u8>,
    presented_once: Cell<bool>,
    /// The parent a host moved this page to while it stayed on screen for its replacement.
    parked: Cell<Option<isize>>,
    /// The window is gone (its host destroyed it).
    destroyed: Cell<bool>,
    presenter: Arc<Presenter>,
}

/// A presenter attach running on the render thread, which must never touch a destroyed window.
#[derive(Default)]
struct Presenter {
    /// Cleared before the window goes: an attach not yet started skips, one that finishes
    /// detaches what it made.
    alive: std::sync::atomic::AtomicBool,
    running: std::sync::atomic::AtomicBool,
    /// The surface the last attach made, for a teardown its completion never reached.
    surface: std::sync::atomic::AtomicU64,
}

impl Page {
    /// A content window filling `parent`, shown, as a new WebView2 controller is: hosts move
    /// and hide it from there. Pages with the same `worker` key share one View isolate.
    pub fn new(
        token: i64,
        shell: Arc<dyn Shell>,
        worker: &str,
        logic: bool,
        parent: isize,
        scheme: Scheme,
    ) -> Result<Rc<Self>, String> {
        lingxia_view::text::fonts::set_system_fonts(fonts::system_fonts);
        let dispatcher = Dispatcher::current()?;
        let worker = ViewWorker::shared(worker)?;
        let hwnd = view::create(parent)?;
        video::install();
        let host = ViewHost::new(WindowsAdapter::new(token, shell, dispatcher), worker, logic);
        let _ = host.adapter().host.set(host.downgrade());
        host.adapter().hwnd.store(hwnd, Ordering::Release);
        let page = Rc::new(Self {
            host,
            hwnd,
            input: Default::default(),
            scale: Cell::new(None),
            scheme: Cell::new(scheme),
            attached: Cell::new(false),
            visible: Cell::new(true),
            settling: Cell::new(false),
            shape: Cell::new((0, 0, [0; 4])),
            settle_epoch: Cell::new(0),
            settle_presents: Cell::new(0),
            presented_once: Cell::new(false),
            parked: Cell::new(None),
            destroyed: Cell::new(false),
            presenter: Arc::new(Presenter {
                alive: true.into(),
                ..Default::default()
            }),
        });
        view::register(hwnd, &page);
        PAGES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((hwnd, dispatcher));
        page.host.set_visible(true);
        page.show_window(false);
        let (width, height) = view::client_size(parent);
        page.set_bounds(0, 0, width as i32, height as i32, [0; 4]);
        page.arrive();
        Ok(page)
    }

    pub fn host(&self) -> &ViewHost<WindowsAdapter> {
        &self.host
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd
    }

    /// Window pixels per point of the page.
    pub fn window_scale(&self) -> f32 {
        self.shown_scale()
    }

    fn window(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    /// Position in the parent's client area, device pixels, clipped to corner radii
    /// `[top-left, top-right, bottom-right, bottom-left]`.
    pub fn set_bounds(&self, left: i32, top: i32, width: i32, height: i32, radii: [i32; 4]) {
        self.shape.set((width, height, radii));
        // SAFETY: this thread's window.
        unsafe {
            let _ = SetWindowPos(
                self.window(),
                None,
                left,
                top,
                width.max(0),
                height.max(0),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.apply_region();
        self.report();
    }

    /// The window's corners, or a single interior pixel while it settles: a window nothing of
    /// which shows is occluded and never presents.
    fn apply_region(&self) {
        let (width, height, radii) = self.shape.get();
        let region = if self.settling.get() {
            let (x, y) = (width.max(1) / 2, height.max(1) / 2);
            // SAFETY: a plain rectangular region, owned by the window once set.
            Some(unsafe { CreateRectRgn(x, y, x + 1, y + 1) })
        } else {
            corner_region(width, height, radii)
        };
        // SAFETY: this thread's window, which owns a region it is given.
        let _ = unsafe { SetWindowRgn(self.window(), region, true) };
    }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) == visible {
            return;
        }
        self.host.set_visible(visible);
        if visible {
            leaving().retain(|(page, _)| *page != self.hwnd);
            if let Some(parent) = self.parked.take() {
                self.set_parent(parent);
            }
            self.report();
            // A page shown again before it left the screen needs no settling.
            // SAFETY: this thread's window.
            if !unsafe { IsWindowVisible(self.window()) }.as_bool() {
                self.arrive();
            }
        } else {
            self.settled();
            self.leave();
        }
    }

    fn show_window(&self, shown: bool) {
        // SAFETY: this thread's window.
        unsafe {
            let _ = ShowWindow(
                self.window(),
                if shown { SW_SHOWNOACTIVATE } else { SW_HIDE },
            );
        }
        if !shown && let Some(parent) = self.parked.take() {
            self.set_parent(parent);
        }
    }

    /// Come on screen without a flash. A frame presented while hidden is lost on showing, so
    /// the page shows clipped to one pixel, over the page it replaces, until a frame of its
    /// present geometry reaches the screen.
    fn arrive(&self) {
        // Nothing shows in a hidden window, which never presents either.
        // SAFETY: this thread's window and its top-level ancestor.
        if !unsafe { IsWindowVisible(GetAncestor(self.window(), GA_ROOT)) }.as_bool() {
            self.show_window(true);
            return;
        }
        // A failed page presents nothing to wait for: its recovery panel shows at once, and
        // the page it replaces leaves now rather than over it.
        if recovery::covers(self.hwnd) {
            self.show_window(true);
            if settling().is_empty() {
                release_left();
            }
            return;
        }
        let mut queue = settling();
        if !queue.contains(&self.hwnd) {
            queue.push(self.hwnd);
        }
        drop(queue);
        let epoch = self.settle_epoch.get() + 1;
        self.settle_epoch.set(epoch);
        self.settle_presents.set(0);
        self.settling.set(true);
        self.apply_region();
        self.show_window(true);
        self.host.redraw();
        let wait = if self.presented_once.get() {
            std::time::Duration::from_millis(300)
        } else {
            std::time::Duration::from_secs(2)
        };
        let hwnd = self.hwnd;
        self.host.adapter().dispatcher.post_after(
            wait,
            Box::new(move || {
                if let Some(page) = view::page_for(hwnd)
                    && page.settle_epoch.get() == epoch
                {
                    log::warn!(target: "lingxia_view", "page shown before a frame of its geometry presented");
                    page.settled();
                }
            }),
        );
    }

    /// A `width`×`height` frame at `scale` reached the screen. While settling, only frames
    /// for the window's present geometry count, and the first of those may have been drawn
    /// hidden.
    pub(crate) fn presented(&self, width: u32, height: u32, scale: f32) {
        self.presented_once.set(true);
        if !self.settling.get() || !self.visible.get() {
            return;
        }
        let current = self
            .host
            .adapter()
            .facts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|facts| (facts.width, facts.height, facts.scale));
        if current != Some((width, height, scale)) {
            return;
        }
        let presents = self.settle_presents.get() + 1;
        self.settle_presents.set(presents);
        if presents < 2 {
            self.host.redraw();
        } else {
            self.settled();
        }
    }

    /// Settling ends: the whole page shows, and once no page settles, hidden pages leave.
    fn settled(&self) {
        self.settle_epoch.set(self.settle_epoch.get() + 1);
        let done = {
            let mut queue = settling();
            queue.retain(|page| *page != self.hwnd);
            queue.is_empty()
        };
        if !self.settling.replace(false) {
            return;
        }
        self.apply_region();
        if done {
            release_left();
        }
    }

    /// Leave the screen once no page settles over the place this one keeps. Hosts show the
    /// page replacing this one before or shortly after hiding it, so a page that never comes
    /// is not waited for long.
    fn leave(&self) {
        let hwnd = self.hwnd;
        let dispatcher = self.host.adapter().dispatcher;
        leaving().push((hwnd, dispatcher));
        dispatcher.post_after(
            std::time::Duration::from_millis(150),
            Box::new(move || {
                if settling().is_empty() {
                    leaving().retain(|(page, _)| *page != hwnd);
                    hide_left(hwnd);
                }
            }),
        );
    }

    pub fn set_parent(&self, parent: isize) {
        // A hidden page still on screen for the page replacing it is parked once it leaves.
        // SAFETY: this thread's window.
        if !self.visible.get() && unsafe { IsWindowVisible(self.window()) }.as_bool() {
            self.parked.set(Some(parent));
            return;
        }
        self.parked.set(None);
        // SAFETY: this thread's window into a live host window.
        let previous = unsafe { SetParent(self.window(), Some(HWND(parent as *mut _))) };
        self.report();
        // A page moved on screen shows what it drew for its old parent until it presents anew.
        let moved = previous.is_ok_and(|previous| previous.0 as isize != parent);
        // SAFETY: this thread's window.
        if moved && self.visible.get() && unsafe { IsWindowVisible(self.window()) }.as_bool() {
            self.arrive();
        }
    }

    /// The window this page sits in now.
    pub fn parent_window(&self) -> isize {
        // SAFETY: this thread's window.
        unsafe { GetParent(self.window()) }.map_or(0, |parent| parent.0 as isize)
    }

    /// Move to `parent` now, as a host does before destroying the current parent: a hidden page
    /// still on screen for its replacement leaves the screen first.
    pub fn park(&self, parent: isize) {
        if !self.visible.get() {
            leaving().retain(|(page, _)| *page != self.hwnd);
            self.parked.set(None);
            // SAFETY: this thread's window.
            let _ = unsafe { ShowWindow(self.window(), SW_HIDE) };
        }
        self.set_parent(parent);
    }

    pub fn bring_to_front(&self) {
        // SAFETY: this thread's window.
        let _ = unsafe {
            SetWindowPos(
                self.window(),
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    pub fn focus(&self) {
        // SAFETY: this thread's window.
        let _ = unsafe { SetFocus(Some(self.window())) };
    }

    pub fn set_scale(&self, scale: Option<f32>) {
        let scale = scale.filter(|s| s.is_finite() && *s > 0.0);
        if self.scale.replace(scale) != scale {
            self.report();
        }
    }

    pub fn set_scheme(&self, scheme: Scheme) {
        if self.scheme.replace(scheme) != scheme {
            self.report();
        }
    }

    /// The system's appearance or settings changed.
    pub fn settings_changed(&self) {
        self.report();
    }

    pub(crate) fn dark(&self) -> bool {
        match self.scheme.get() {
            Scheme::Auto => environment::system_dark(),
            Scheme::Light => false,
            Scheme::Dark => true,
        }
    }

    /// Measure the window and tell the host; binds a presenter once it can.
    pub(crate) fn report(&self) {
        let (window_width, window_height) = view::client_size(self.hwnd);
        if window_width == 0 || window_height == 0 {
            return;
        }
        let simulated = simulation();
        let shown = self.shown_scale();
        let scale = simulated.as_ref().map_or(shown, |s| s.pixel_ratio);
        let (width, height) = match &simulated {
            Some(s) => (
                (s.viewport[0] * scale).round() as u32,
                (s.viewport[1] * scale).round() as u32,
            ),
            None => (window_width, window_height),
        };
        let text_scale = simulated
            .as_ref()
            .filter(|s| s.text_scale != 1.0)
            .map(|s| vec![[0.0, 0.0], [16384.0, 16384.0 * s.text_scale]]);
        let facts = DeviceFacts {
            width,
            height,
            scale,
            os: "windows".into(),
            dark: self.dark(),
            system_insets: simulated
                .as_ref()
                .map_or([0.0; 4], |s| s.safe_area.map(|v| v * scale)),
            chrome_insets: simulated.as_ref().map_or([0.0; 4], |s| {
                let [navigation, tabs] = self.host.adapter().shell.bars(self.host.adapter().token);
                [
                    if navigation {
                        s.safe_area[0] + s.navigation_bar
                    } else {
                        0.0
                    },
                    0.0,
                    if tabs { s.tab_bar_reach } else { 0.0 },
                    0.0,
                ]
                .map(|v| v * scale)
            }),
            keyboard: 0.0,
            smallest_width: simulated
                .as_ref()
                .map_or_else(|| self.smallest_width(shown), |s| s.smallest_width),
            locale: environment::locale(),
            fine_pointer: true,
            animation_scale: 1.0,
            reduce_motion: environment::reduce_motion(),
            high_contrast: simulated
                .as_ref()
                .map_or_else(environment::high_contrast, |s| s.high_contrast),
            text_scale,
            hour_cycle: Some(environment::hour_cycle()),
            first_weekday: Some(environment::first_weekday()),
        };
        // A simulated device lays out at its own pixels; frames draw at the window's.
        let raster = simulated
            .is_some()
            .then_some(shown)
            .filter(|shown| (shown - scale).abs() > f32::EPSILON);
        let changed = {
            let mut current = self
                .host
                .adapter()
                .raster_scale
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            std::mem::replace(&mut *current, raster) != raster
        };
        self.measured(facts);
        if changed {
            self.host.redraw();
        }
        if !self.attached.get() && self.visible.get() {
            self.attached.set(true);
            self.attach(window_width, window_height);
        }
    }

    /// Pixels per point of the page: a simulated device's ratio, else the window's.
    pub(crate) fn scale(&self) -> f32 {
        simulation().map_or_else(|| self.shown_scale(), |s| s.pixel_ratio)
    }

    /// Window pixels per point of the page.
    pub(crate) fn shown_scale(&self) -> f32 {
        if let Some(s) = simulation() {
            let (width, _) = view::client_size(self.hwnd);
            if width > 0 {
                return width as f32 / s.viewport[0];
            }
        }
        // SAFETY: this thread's window.
        let dpi = unsafe { GetDpiForWindow(self.window()) };
        self.scale
            .get()
            .unwrap_or(if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 })
    }

    /// A position in window pixels as the page's pixels.
    pub(crate) fn page_pixels(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let factor = self.scale() / self.shown_scale();
        (x * factor, y * factor)
    }

    /// The monitor's shorter side, logical pixels.
    fn smallest_width(&self, scale: f32) -> f32 {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: the window's monitor into a sized struct.
        let ok = unsafe {
            let monitor = MonitorFromWindow(self.window(), MONITOR_DEFAULTTONEAREST);
            GetMonitorInfoW(monitor, &mut info).as_bool()
        };
        if !ok {
            return 0.0;
        }
        let rect = info.rcMonitor;
        ((rect.right - rect.left).min(rect.bottom - rect.top)) as f32 / scale
    }

    fn measured(&self, facts: DeviceFacts) {
        let resized = {
            let mut current = self
                .host
                .adapter()
                .facts
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if current.as_ref() == Some(&facts) {
                return;
            }
            let resized = current.as_ref().is_none_or(|old| {
                (old.width, old.height, old.scale) != (facts.width, facts.height, facts.scale)
            });
            *current = Some(facts);
            resized
        };
        if resized {
            // A settling page waits for frames of its new geometry.
            self.settle_presents.set(0);
            self.host.surface_changed();
        } else {
            self.host.request_frame();
        }
    }

    /// Bind the content window to a presenter on the render thread; frames wait for it, then
    /// use the CPU when it fails.
    fn attach(&self, width: u32, height: u32) {
        use lingxia_view::renderer::gpu::SurfaceSource;
        use lingxia_view::renderer::surfaces::{self, Target};
        let adapter = self.host.adapter();
        let Some(window) = surface::Hwnd::new(self.hwnd) else {
            return;
        };
        let tree = composition::tree(self.hwnd);
        let existing = adapter.gpu.load(Ordering::Acquire);
        *adapter.target.lock().unwrap_or_else(|p| p.into_inner()) = Target::Waiting;
        let host = self.host.downgrade();
        let dispatcher = adapter.dispatcher;
        let started = std::time::Instant::now();
        let presenter = self.presenter.clone();
        presenter.running.store(true, Ordering::Release);
        render::run(move || {
            if !presenter.alive.load(Ordering::Acquire) {
                presenter.running.store(false, Ordering::Release);
                return;
            }
            let source = match tree {
                Some(tree) => SurfaceSource::Visual(tree),
                None => SurfaceSource::window(Arc::new(window)),
            };
            let attached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                surfaces::attach(source, width, height, existing)
            }));
            if let Ok(Ok(id)) = attached
                && !presenter.alive.load(Ordering::Acquire)
            {
                surfaces::detach(id);
                presenter.running.store(false, Ordering::Release);
                return;
            }
            if let Ok(Ok(id)) = attached {
                presenter.surface.store(id, Ordering::Release);
            }
            presenter.running.store(false, Ordering::Release);
            let attached = attached.unwrap_or_else(|panic| {
                Err(panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "presenter setup panicked".into()))
            });
            let ms = started.elapsed().as_millis();
            dispatcher.post(Box::new(move || {
                let Some(host) = host.upgrade() else { return };
                let adapter = host.adapter();
                let target = match attached {
                    Ok(id) => {
                        adapter.gpu.store(id, Ordering::Release);
                        let (record, backend) = surfaces::describe(id).unwrap_or_default();
                        adapter::log(&format!(
                            "backend={backend} surface={id} attachMs={ms} adapter={record}"
                        ));
                        Target::Ready
                    }
                    Err(error) => {
                        adapter::log(&format!("GPU unavailable; using CPU renderer: {error}"));
                        Target::Unavailable
                    }
                };
                *adapter.target.lock().unwrap_or_else(|p| p.into_inner()) = target;
                host.surface_changed();
            }));
        });
    }

    fn gpu(&self) -> u64 {
        self.host.adapter().gpu.load(Ordering::Acquire)
    }

    /// The newest GPU frame as a PNG; `None` on the CPU path.
    pub fn screenshot_png(&self) -> Result<Option<Vec<u8>>, String> {
        let id = self.gpu();
        if id == 0 {
            return Ok(None);
        }
        render::run_now(move || lingxia_view::renderer::surfaces::screenshot_png(id))
            .ok_or("UI_SURFACE: render thread gone")?
    }

    /// No attach runs against the window from now on. Pumps this thread's messages while one
    /// finishes, since creating a swap chain can wait on the window's thread.
    fn retire_presenter(&self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage, TranslateMessage,
            WM_QUIT,
        };
        self.presenter.alive.store(false, Ordering::Release);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut quit = None;
        while self.presenter.running.load(Ordering::Acquire) && std::time::Instant::now() < deadline
        {
            let mut msg = MSG::default();
            // SAFETY: drains this thread's queue; a WM_QUIT seen here is posted again after.
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        quit = Some(msg.wParam.0 as i32);
                        continue;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        if let Some(code) = quit {
            // SAFETY: re-posts the quit this loop took.
            unsafe { PostQuitMessage(code) };
        }
    }

    /// Release the presenter bound to the window; the window must still exist.
    fn release_surface(&self) {
        let current = self.host.adapter().gpu.swap(0, Ordering::AcqRel);
        let last = self.presenter.surface.swap(0, Ordering::AcqRel);
        let stale = (last != current).then_some(last);
        for surface in [Some(current), stale].into_iter().flatten() {
            if surface != 0 {
                render::run_now(move || lingxia_view::renderer::surfaces::detach(surface));
            }
        }
    }

    /// The window is being destroyed under the page: frames wait from now on.
    pub(crate) fn window_destroyed(&self) {
        if self.destroyed.replace(true) {
            return;
        }
        log::warn!(target: "lingxia_view", "GPU page window destroyed by its host");
        self.settled();
        leaving().retain(|(page, _)| *page != self.hwnd);
        view::unregister(self.hwnd);
        self.retire_presenter();
        *self
            .host
            .adapter()
            .target
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = lingxia_view::renderer::surfaces::Target::Waiting;
        self.release_surface();
        self.host.surface_lost();
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        PAGES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retain(|(page, _)| *page != self.hwnd);
        self.settled();
        leaving().retain(|(page, _)| *page != self.hwnd);
        view::unregister(self.hwnd);
        vsync::cancel(self.host.adapter().token);
        vsync::cancel(self.host.adapter().fling_key());
        self.host.dispose();
        // The presenter's swap chain is bound to the window: release it first.
        self.retire_presenter();
        self.release_surface();
        if !self.destroyed.get() {
            // SAFETY: this thread's window, destroyed once.
            let _ = unsafe { DestroyWindow(self.window()) };
        }
    }
}

/// `width`×`height` with each corner cut to its radius; `None` when no corner is round.
fn corner_region(width: i32, height: i32, radii: [i32; 4]) -> Option<HRGN> {
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateEllipticRgn, CreateRectRgn, RGN_DIFF};
    if radii.iter().all(|r| *r <= 0) || width <= 0 || height <= 0 {
        return None;
    }
    // SAFETY: GDI regions this function owns until it hands the result back.
    unsafe {
        let region = CreateRectRgn(0, 0, width, height);
        for (corner, radius) in radii.into_iter().enumerate() {
            let r = radius.min(width / 2).min(height / 2);
            if r <= 0 {
                continue;
            }
            let x = if corner == 1 || corner == 2 {
                width - r
            } else {
                0
            };
            let y = if corner >= 2 { height - r } else { 0 };
            let cx = if corner == 1 || corner == 2 {
                width - 2 * r
            } else {
                0
            };
            let cy = if corner >= 2 { height - 2 * r } else { 0 };
            let square = CreateRectRgn(x, y, x + r, y + r);
            // A pixel past the arc on each side: GDI drops the arc's partly covered pixels,
            // which a host's anti-aliased corner overlay blends over the parent window.
            let arc = CreateEllipticRgn(cx - 1, cy - 1, cx + 2 * r + 2, cy + 2 * r + 2);
            CombineRgn(Some(square), Some(square), Some(arc), RGN_DIFF);
            CombineRgn(Some(region), Some(region), Some(square), RGN_DIFF);
            let _ = DeleteObject(square.into());
            let _ = DeleteObject(arc.into());
        }
        Some(region)
    }
}
