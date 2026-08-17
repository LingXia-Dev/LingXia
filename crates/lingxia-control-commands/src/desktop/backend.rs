//! Where a `desktop` command actually runs.
//!
//! Two mounts, one command table. `lxdev` runs these in its own process: it is
//! a development tool, a developer grants it Accessibility once, and there is
//! no app to route through. A shipped product must not — macOS attributes
//! Accessibility and Screen Recording to the responsible process, so a client
//! invoked from a terminal borrows *that terminal's* grants, answers
//! differently depending on which terminal launched it, and shows the user an
//! entry in System Settings naming iTerm instead of the product. Sent to the
//! app, the grant is the product's own.

use lingxia_control_protocol::methods::desktop as method;
use lingxia_device_io as device_io;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::guard::decode_failure;
use crate::transport::Transport;

pub enum Backend<'a> {
    /// Run in this process. Correct for `lxdev`, wrong for a product — so the
    /// native implementation it dispatches into is behind `desktop-local`, and
    /// a product that only ever forwards never links it.
    #[cfg(feature = "desktop-local")]
    Local,
    /// Ask the running product to run it.
    App(&'a dyn Transport),
}

impl Backend<'_> {
    pub fn doctor(&self) -> device_io::Result<device_io::Doctor> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => Ok(device_io::doctor()),
            Self::App(_) => self.call(method::DOCTOR, ()),
        }
    }

    pub fn permissions(&self) -> device_io::Result<device_io::Permissions> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => Ok(device_io::permissions()),
            Self::App(_) => self.call(method::PERMISSIONS, ()),
        }
    }

    pub fn request_permissions(&self) -> device_io::Result<device_io::Permissions> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => Ok(device_io::request_permissions()),
            Self::App(_) => self.call(method::REQUEST_PERMISSIONS, ()),
        }
    }

    pub fn displays(&self) -> device_io::Result<Vec<device_io::Display>> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::displays(),
            Self::App(_) => self.call(method::DISPLAYS, ()),
        }
    }

    pub fn windows(
        &self,
        query: &device_io::WindowQuery,
    ) -> device_io::Result<Vec<device_io::Window>> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::windows(query),
            Self::App(_) => self.call(
                method::WINDOWS,
                device_io::wire::Windows {
                    query: query.clone(),
                },
            ),
        }
    }

    pub fn screenshot(
        &self,
        target: device_io::CaptureTarget,
    ) -> device_io::Result<device_io::Capture> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::capture::snapshot(target),
            Self::App(_) => self.call(method::SCREENSHOT, device_io::wire::Screenshot { target }),
        }
    }

    pub fn pixel(&self, x: i32, y: i32) -> device_io::Result<device_io::Pixel> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::capture::pixel(x, y),
            Self::App(_) => self.call(method::PIXEL, device_io::wire::Point { x, y }),
        }
    }

    pub fn wait_window(
        &self,
        query: &device_io::WindowQuery,
        visible: Option<bool>,
        timeout_ms: u64,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::wait_window(query, visible, timeout_ms),
            Self::App(_) => self.call(
                method::WAIT_WINDOW,
                device_io::wire::WaitWindow {
                    query: query.clone(),
                    visible,
                    timeout_ms,
                },
            ),
        }
    }

    pub fn wait_pixel(
        &self,
        x: i32,
        y: i32,
        hex: &str,
        tolerance: u8,
        timeout_ms: u64,
    ) -> device_io::Result<device_io::Pixel> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::capture::wait_pixel(x, y, hex, tolerance, timeout_ms),
            Self::App(_) => self.call(
                method::WAIT_PIXEL,
                device_io::wire::WaitPixel {
                    x,
                    y,
                    hex: hex.to_string(),
                    tolerance,
                    timeout_ms,
                },
            ),
        }
    }

    pub fn window_status(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::STATUS,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::status,
            target,
        )
    }

    pub fn window_focus(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::FOCUS,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::focus,
            target,
        )
    }

    pub fn window_activate(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::ACTIVATE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::activate,
            target,
        )
    }

    pub fn window_raise(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::RAISE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::raise,
            target,
        )
    }

    pub fn window_minimize(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::MINIMIZE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::minimize,
            target,
        )
    }

    pub fn window_restore(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::RESTORE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::restore,
            target,
        )
    }

    pub fn window_maximize(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::MAXIMIZE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::maximize,
            target,
        )
    }

    pub fn window_close(
        &self,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        self.window(
            method::window::CLOSE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::window::close,
            target,
        )
    }

    pub fn window_move(
        &self,
        target: &device_io::WindowTarget,
        x: i32,
        y: i32,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::window::move_to(target, x, y),
            Self::App(_) => self.call(
                method::window::MOVE,
                device_io::wire::WindowMove {
                    target: target.clone(),
                    x,
                    y,
                },
            ),
        }
    }

    pub fn window_move_display(
        &self,
        target: &device_io::WindowTarget,
        display_id: &str,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::window::move_to_display(target, display_id),
            Self::App(_) => self.call(
                method::window::MOVE_DISPLAY,
                device_io::wire::WindowMoveDisplay {
                    target: target.clone(),
                    display_id: display_id.to_string(),
                },
            ),
        }
    }

    pub fn window_resize(
        &self,
        target: &device_io::WindowTarget,
        width: i32,
        height: i32,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::window::resize(target, width, height),
            Self::App(_) => self.call(
                method::window::RESIZE,
                device_io::wire::WindowResize {
                    target: target.clone(),
                    width,
                    height,
                },
            ),
        }
    }

    pub fn window_set_always_on_top(
        &self,
        target: &device_io::WindowTarget,
        on: bool,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::window::set_always_on_top(target, on),
            Self::App(_) => self.call(
                method::window::SET_ALWAYS_ON_TOP,
                device_io::wire::WindowAlwaysOnTop {
                    target: target.clone(),
                    on,
                },
            ),
        }
    }

    pub fn pointer_move(
        &self,
        x: i32,
        y: i32,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::pointer_move(x, y, target),
            Self::App(_) => self.call(
                method::pointer::MOVE,
                device_io::wire::PointerMove {
                    x,
                    y,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn pointer_down(
        &self,
        x: i32,
        y: i32,
        button: device_io::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::pointer_down(x, y, button, target),
            Self::App(_) => self.call(
                method::pointer::DOWN,
                device_io::wire::PointerButton {
                    x,
                    y,
                    button,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn pointer_up(
        &self,
        x: i32,
        y: i32,
        button: device_io::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::pointer_up(x, y, button, target),
            Self::App(_) => self.call(
                method::pointer::UP,
                device_io::wire::PointerButton {
                    x,
                    y,
                    button,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn pointer_click(
        &self,
        x: i32,
        y: i32,
        button: device_io::MouseButton,
        count: u32,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::pointer_click(x, y, button, count, target),
            Self::App(_) => self.call(
                method::pointer::CLICK,
                device_io::wire::PointerClick {
                    x,
                    y,
                    button,
                    count,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn pointer_scroll(
        &self,
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::pointer_scroll(x, y, dx, dy, target),
            Self::App(_) => self.call(
                method::pointer::SCROLL,
                device_io::wire::PointerScroll {
                    x,
                    y,
                    dx,
                    dy,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn pointer_drag(
        &self,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        button: device_io::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => {
                device_io::input::pointer_drag(from_x, from_y, to_x, to_y, button, target)
            }
            Self::App(_) => self.call(
                method::pointer::DRAG,
                device_io::wire::PointerDrag {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    button,
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn key_type(
        &self,
        text: &str,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::key_type(text, target),
            Self::App(_) => self.call(
                method::key::TYPE,
                device_io::wire::KeyText {
                    text: text.to_string(),
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn key_down(
        &self,
        name: &str,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        self.key(
            method::key::DOWN,
            #[cfg(feature = "desktop-local")]
            device_io::input::key_down,
            name,
            target,
            window_id,
        )
    }

    pub fn key_up(
        &self,
        name: &str,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        self.key(
            method::key::UP,
            #[cfg(feature = "desktop-local")]
            device_io::input::key_up,
            name,
            target,
            window_id,
        )
    }

    pub fn key_press(
        &self,
        name: &str,
        modifiers: &[device_io::Modifier],
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::input::key_press(name, modifiers, target),
            Self::App(_) => self.call(
                method::key::PRESS,
                device_io::wire::KeyPress {
                    name: name.to_string(),
                    modifiers: modifiers.to_vec(),
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    pub fn ax_tree(
        &self,
        window_id: &str,
        depth: Option<u32>,
        max_nodes: Option<usize>,
    ) -> device_io::Result<device_io::AxNode> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::ax::tree(window_id, depth, max_nodes),
            Self::App(_) => self.call(
                method::ax::TREE,
                device_io::wire::AxTree {
                    window_id: window_id.to_string(),
                    depth,
                    max_nodes,
                },
            ),
        }
    }

    pub fn ax_hit_test(&self, x: i32, y: i32) -> device_io::Result<device_io::AxNode> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::ax::hit_test(x, y),
            Self::App(_) => self.call(method::ax::HIT_TEST, device_io::wire::Point { x, y }),
        }
    }

    pub fn ax_query(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
        all: bool,
        index: Option<usize>,
    ) -> device_io::Result<Vec<device_io::AxNode>> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::ax::query(window_id, query, all, index),
            Self::App(_) => self.call(
                method::ax::QUERY,
                device_io::wire::AxSearch {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                    all,
                    index,
                },
            ),
        }
    }

    pub fn ax_invoke(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::INVOKE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::ax::invoke,
            window_id,
            query,
        )
    }

    pub fn ax_focus(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::FOCUS,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::ax::focus,
            window_id,
            query,
        )
    }

    pub fn ax_select(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::SELECT,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::ax::select,
            window_id,
            query,
        )
    }

    pub fn ax_expand(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::EXPAND,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::ax::expand,
            window_id,
            query,
        )
    }

    pub fn ax_collapse(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::COLLAPSE,
            #[cfg(feature = "desktop-local")]
            #[cfg(feature = "desktop-local")]
            device_io::ax::collapse,
            window_id,
            query,
        )
    }

    pub fn ax_scroll_into_view(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        self.ax(
            method::ax::SCROLL_INTO_VIEW,
            #[cfg(feature = "desktop-local")]
            device_io::ax::scroll_into_view,
            window_id,
            query,
        )
    }

    pub fn ax_set_value(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
        value: &str,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::ax::set_value(window_id, query, value),
            Self::App(_) => self.call(
                method::ax::SET_VALUE,
                device_io::wire::AxSetValue {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                    value: value.to_string(),
                },
            ),
        }
    }

    pub fn ax_wait(
        &self,
        window_id: &str,
        query: &device_io::AxQuery,
        state: &str,
        timeout_ms: u64,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::ax::wait(window_id, query, state, timeout_ms),
            Self::App(_) => self.call(
                method::ax::WAIT,
                device_io::wire::AxWait {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                    state: state.to_string(),
                    timeout_ms,
                },
            ),
        }
    }

    pub fn clipboard_get(&self) -> device_io::Result<device_io::Clipboard> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::clipboard::get(),
            Self::App(_) => self.call(method::clipboard::GET, ()),
        }
    }

    pub fn clipboard_clear(&self) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::clipboard::clear(),
            Self::App(_) => self.call(method::clipboard::CLEAR, ()),
        }
    }

    pub fn clipboard_paste(&self) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::clipboard::paste(),
            Self::App(_) => self.call(method::clipboard::PASTE, ()),
        }
    }

    pub fn clipboard_set(&self, text: &str) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::clipboard::set(text),
            Self::App(_) => self.call(
                method::clipboard::SET,
                device_io::wire::ClipboardSet {
                    text: text.to_string(),
                },
            ),
        }
    }

    pub fn app_launch(
        &self,
        app: &str,
        args: &[String],
        wait_window: Option<&str>,
        timeout_ms: u64,
    ) -> device_io::Result<device_io::LaunchResult> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::app::launch(app, args, wait_window, timeout_ms),
            Self::App(_) => self.call(
                method::app::LAUNCH,
                device_io::wire::AppLaunch {
                    app: app.to_string(),
                    args: args.to_vec(),
                    wait_window: wait_window.map(str::to_string),
                    timeout_ms,
                },
            ),
        }
    }

    pub fn app_quit(
        &self,
        target: device_io::QuitTarget,
        force: bool,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::app::quit(target, force),
            Self::App(_) => self.call(
                method::app::QUIT,
                device_io::wire::AppQuit { target, force },
            ),
        }
    }

    pub fn process_list(
        &self,
        filter: Option<&str>,
    ) -> device_io::Result<Vec<device_io::ProcessInfo>> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::process::list(filter),
            Self::App(_) => self.call(
                method::process::LIST,
                device_io::wire::ProcessList {
                    filter: filter.map(str::to_string),
                },
            ),
        }
    }

    pub fn process_kill(&self, pid: u32, force: bool) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => device_io::process::kill(pid, force),
            Self::App(_) => self.call(
                method::process::KILL,
                device_io::wire::ProcessKill { pid, force },
            ),
        }
    }

    /// The window verbs that differ only in which function they name.
    fn window(
        &self,
        name: &str,
        #[cfg(feature = "desktop-local")] action: fn(
            &device_io::WindowTarget,
        )
            -> device_io::Result<device_io::Window>,
        target: &device_io::WindowTarget,
    ) -> device_io::Result<device_io::Window> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => action(target),
            Self::App(_) => self.call(
                name,
                device_io::wire::WindowAction {
                    target: target.clone(),
                },
            ),
        }
    }

    fn key(
        &self,
        name: &str,
        #[cfg(feature = "desktop-local")] action: fn(
            &str,
            Option<u32>,
        ) -> device_io::Result<device_io::Ack>,
        key: &str,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => action(key, target),
            Self::App(_) => self.call(
                name,
                device_io::wire::KeyName {
                    name: key.to_string(),
                    target,
                    window_id: window_id.map(str::to_string),
                },
            ),
        }
    }

    fn ax(
        &self,
        name: &str,
        #[cfg(feature = "desktop-local")] action: fn(
            &str,
            &device_io::AxQuery,
        ) -> device_io::Result<device_io::Ack>,
        window_id: &str,
        query: &device_io::AxQuery,
    ) -> device_io::Result<device_io::Ack> {
        match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => action(window_id, query),
            Self::App(_) => self.call(
                name,
                device_io::wire::AxAction {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                },
            ),
        }
    }

    fn call<A: Serialize, T: DeserializeOwned>(&self, name: &str, args: A) -> device_io::Result<T> {
        let transport = match self {
            #[cfg(feature = "desktop-local")]
            Self::Local => unreachable!("call is only reached on the App backend"),
            Self::App(transport) => transport,
        };
        let params = serde_json::to_value(args)
            .map_err(|error| device_io::Error::Usage(error.to_string()))?;
        let params = (!params.is_null()).then_some(params);
        let result = transport
            .request(name, params)
            .map_err(|error| decode_failure(&error))?;
        serde_json::from_value(result.unwrap_or(Value::Null)).map_err(|error| {
            device_io::Error::Failed(format!("{name} returned an unreadable answer: {error}"))
        })
    }
}
