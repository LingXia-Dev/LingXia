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

use lingxia_device_io as cu;
use lingxia_control_protocol::methods::desktop as method;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::guard::decode_failure;
use crate::transport::Transport;

pub enum Backend<'a> {
    /// Run in this process. Correct for `lxdev`, wrong for a product.
    Local,
    /// Ask the running product to run it.
    App(&'a dyn Transport),
}

impl Backend<'_> {
    pub fn doctor(&self) -> cu::Result<cu::Doctor> {
        match self {
            Self::Local => Ok(cu::doctor()),
            Self::App(_) => self.call(method::DOCTOR, ()),
        }
    }

    pub fn permissions(&self) -> cu::Result<cu::Permissions> {
        match self {
            Self::Local => Ok(cu::permissions()),
            Self::App(_) => self.call(method::PERMISSIONS, ()),
        }
    }

    pub fn request_permissions(&self) -> cu::Result<cu::Permissions> {
        match self {
            Self::Local => Ok(cu::request_permissions()),
            Self::App(_) => self.call(method::REQUEST_PERMISSIONS, ()),
        }
    }

    pub fn displays(&self) -> cu::Result<Vec<cu::Display>> {
        match self {
            Self::Local => cu::displays(),
            Self::App(_) => self.call(method::DISPLAYS, ()),
        }
    }

    pub fn windows(&self, query: &cu::WindowQuery) -> cu::Result<Vec<cu::Window>> {
        match self {
            Self::Local => cu::windows(query),
            Self::App(_) => self.call(
                method::WINDOWS,
                cu::wire::Windows {
                    query: query.clone(),
                },
            ),
        }
    }

    pub fn screenshot(&self, target: cu::CaptureTarget) -> cu::Result<cu::Capture> {
        match self {
            Self::Local => cu::screenshot(target),
            Self::App(_) => self.call(method::SCREENSHOT, cu::wire::Screenshot { target }),
        }
    }

    pub fn pixel(&self, x: i32, y: i32) -> cu::Result<cu::Pixel> {
        match self {
            Self::Local => cu::pixel(x, y),
            Self::App(_) => self.call(method::PIXEL, cu::wire::Point { x, y }),
        }
    }

    pub fn wait_window(
        &self,
        query: &cu::WindowQuery,
        visible: Option<bool>,
        timeout_ms: u64,
    ) -> cu::Result<cu::Window> {
        match self {
            Self::Local => cu::wait_window(query, visible, timeout_ms),
            Self::App(_) => self.call(
                method::WAIT_WINDOW,
                cu::wire::WaitWindow {
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
    ) -> cu::Result<cu::Pixel> {
        match self {
            Self::Local => cu::wait_pixel(x, y, hex, tolerance, timeout_ms),
            Self::App(_) => self.call(
                method::WAIT_PIXEL,
                cu::wire::WaitPixel {
                    x,
                    y,
                    hex: hex.to_string(),
                    tolerance,
                    timeout_ms,
                },
            ),
        }
    }

    pub fn window_status(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::STATUS, cu::window::status, target)
    }

    pub fn window_focus(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::FOCUS, cu::window::focus, target)
    }

    pub fn window_activate(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::ACTIVATE, cu::window::activate, target)
    }

    pub fn window_raise(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::RAISE, cu::window::raise, target)
    }

    pub fn window_minimize(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::MINIMIZE, cu::window::minimize, target)
    }

    pub fn window_restore(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::RESTORE, cu::window::restore, target)
    }

    pub fn window_maximize(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::MAXIMIZE, cu::window::maximize, target)
    }

    pub fn window_close(&self, target: &cu::WindowTarget) -> cu::Result<cu::Window> {
        self.window(method::window::CLOSE, cu::window::close, target)
    }

    pub fn window_move(&self, target: &cu::WindowTarget, x: i32, y: i32) -> cu::Result<cu::Window> {
        match self {
            Self::Local => cu::window::move_to(target, x, y),
            Self::App(_) => self.call(
                method::window::MOVE,
                cu::wire::WindowMove {
                    target: target.clone(),
                    x,
                    y,
                },
            ),
        }
    }

    pub fn window_move_display(
        &self,
        target: &cu::WindowTarget,
        display_id: &str,
    ) -> cu::Result<cu::Window> {
        match self {
            Self::Local => cu::window::move_to_display(target, display_id),
            Self::App(_) => self.call(
                method::window::MOVE_DISPLAY,
                cu::wire::WindowMoveDisplay {
                    target: target.clone(),
                    display_id: display_id.to_string(),
                },
            ),
        }
    }

    pub fn window_resize(
        &self,
        target: &cu::WindowTarget,
        width: i32,
        height: i32,
    ) -> cu::Result<cu::Window> {
        match self {
            Self::Local => cu::window::resize(target, width, height),
            Self::App(_) => self.call(
                method::window::RESIZE,
                cu::wire::WindowResize {
                    target: target.clone(),
                    width,
                    height,
                },
            ),
        }
    }

    pub fn window_set_always_on_top(
        &self,
        target: &cu::WindowTarget,
        on: bool,
    ) -> cu::Result<cu::Window> {
        match self {
            Self::Local => cu::window::set_always_on_top(target, on),
            Self::App(_) => self.call(
                method::window::SET_ALWAYS_ON_TOP,
                cu::wire::WindowAlwaysOnTop {
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
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_move(x, y, target),
            Self::App(_) => self.call(
                method::pointer::MOVE,
                cu::wire::PointerMove {
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
        button: cu::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_down(x, y, button, target),
            Self::App(_) => self.call(
                method::pointer::DOWN,
                cu::wire::PointerButton {
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
        button: cu::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_up(x, y, button, target),
            Self::App(_) => self.call(
                method::pointer::UP,
                cu::wire::PointerButton {
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
        button: cu::MouseButton,
        count: u32,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_click(x, y, button, count, target),
            Self::App(_) => self.call(
                method::pointer::CLICK,
                cu::wire::PointerClick {
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
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_scroll(x, y, dx, dy, target),
            Self::App(_) => self.call(
                method::pointer::SCROLL,
                cu::wire::PointerScroll {
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
        button: cu::MouseButton,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::pointer_drag(from_x, from_y, to_x, to_y, button, target),
            Self::App(_) => self.call(
                method::pointer::DRAG,
                cu::wire::PointerDrag {
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
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::key_type(text, target),
            Self::App(_) => self.call(
                method::key::TYPE,
                cu::wire::KeyText {
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
    ) -> cu::Result<cu::Ack> {
        self.key(
            method::key::DOWN,
            cu::input::key_down,
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
    ) -> cu::Result<cu::Ack> {
        self.key(method::key::UP, cu::input::key_up, name, target, window_id)
    }

    pub fn key_press(
        &self,
        name: &str,
        modifiers: &[cu::Modifier],
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::input::key_press(name, modifiers, target),
            Self::App(_) => self.call(
                method::key::PRESS,
                cu::wire::KeyPress {
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
    ) -> cu::Result<cu::AxNode> {
        match self {
            Self::Local => cu::ax::tree(window_id, depth, max_nodes),
            Self::App(_) => self.call(
                method::ax::TREE,
                cu::wire::AxTree {
                    window_id: window_id.to_string(),
                    depth,
                    max_nodes,
                },
            ),
        }
    }

    pub fn ax_hit_test(&self, x: i32, y: i32) -> cu::Result<cu::AxNode> {
        match self {
            Self::Local => cu::ax::hit_test(x, y),
            Self::App(_) => self.call(method::ax::HIT_TEST, cu::wire::Point { x, y }),
        }
    }

    pub fn ax_query(
        &self,
        window_id: &str,
        query: &cu::AxQuery,
        all: bool,
        index: Option<usize>,
    ) -> cu::Result<Vec<cu::AxNode>> {
        match self {
            Self::Local => cu::ax::query(window_id, query, all, index),
            Self::App(_) => self.call(
                method::ax::QUERY,
                cu::wire::AxSearch {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                    all,
                    index,
                },
            ),
        }
    }

    pub fn ax_invoke(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(method::ax::INVOKE, cu::ax::invoke, window_id, query)
    }

    pub fn ax_focus(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(method::ax::FOCUS, cu::ax::focus, window_id, query)
    }

    pub fn ax_select(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(method::ax::SELECT, cu::ax::select, window_id, query)
    }

    pub fn ax_expand(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(method::ax::EXPAND, cu::ax::expand, window_id, query)
    }

    pub fn ax_collapse(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(method::ax::COLLAPSE, cu::ax::collapse, window_id, query)
    }

    pub fn ax_scroll_into_view(&self, window_id: &str, query: &cu::AxQuery) -> cu::Result<cu::Ack> {
        self.ax(
            method::ax::SCROLL_INTO_VIEW,
            cu::ax::scroll_into_view,
            window_id,
            query,
        )
    }

    pub fn ax_set_value(
        &self,
        window_id: &str,
        query: &cu::AxQuery,
        value: &str,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::ax::set_value(window_id, query, value),
            Self::App(_) => self.call(
                method::ax::SET_VALUE,
                cu::wire::AxSetValue {
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
        query: &cu::AxQuery,
        state: &str,
        timeout_ms: u64,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::ax::wait(window_id, query, state, timeout_ms),
            Self::App(_) => self.call(
                method::ax::WAIT,
                cu::wire::AxWait {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                    state: state.to_string(),
                    timeout_ms,
                },
            ),
        }
    }

    pub fn clipboard_get(&self) -> cu::Result<cu::Clipboard> {
        match self {
            Self::Local => cu::clipboard::get(),
            Self::App(_) => self.call(method::clipboard::GET, ()),
        }
    }

    pub fn clipboard_clear(&self) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::clipboard::clear(),
            Self::App(_) => self.call(method::clipboard::CLEAR, ()),
        }
    }

    pub fn clipboard_paste(&self) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::clipboard::paste(),
            Self::App(_) => self.call(method::clipboard::PASTE, ()),
        }
    }

    pub fn clipboard_set(&self, text: &str) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::clipboard::set(text),
            Self::App(_) => self.call(
                method::clipboard::SET,
                cu::wire::ClipboardSet {
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
    ) -> cu::Result<cu::LaunchResult> {
        match self {
            Self::Local => cu::app::launch(app, args, wait_window, timeout_ms),
            Self::App(_) => self.call(
                method::app::LAUNCH,
                cu::wire::AppLaunch {
                    app: app.to_string(),
                    args: args.to_vec(),
                    wait_window: wait_window.map(str::to_string),
                    timeout_ms,
                },
            ),
        }
    }

    pub fn app_quit(&self, target: cu::QuitTarget, force: bool) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::app::quit(target, force),
            Self::App(_) => self.call(method::app::QUIT, cu::wire::AppQuit { target, force }),
        }
    }

    pub fn process_list(&self, filter: Option<&str>) -> cu::Result<Vec<cu::ProcessInfo>> {
        match self {
            Self::Local => cu::process::list(filter),
            Self::App(_) => self.call(
                method::process::LIST,
                cu::wire::ProcessList {
                    filter: filter.map(str::to_string),
                },
            ),
        }
    }

    pub fn process_kill(&self, pid: u32, force: bool) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => cu::process::kill(pid, force),
            Self::App(_) => self.call(method::process::KILL, cu::wire::ProcessKill { pid, force }),
        }
    }

    /// The window verbs that differ only in which function they name.
    fn window(
        &self,
        name: &str,
        action: fn(&cu::WindowTarget) -> cu::Result<cu::Window>,
        target: &cu::WindowTarget,
    ) -> cu::Result<cu::Window> {
        match self {
            Self::Local => action(target),
            Self::App(_) => self.call(
                name,
                cu::wire::WindowAction {
                    target: target.clone(),
                },
            ),
        }
    }

    fn key(
        &self,
        name: &str,
        action: fn(&str, Option<u32>) -> cu::Result<cu::Ack>,
        key: &str,
        target: Option<u32>,
        window_id: Option<&str>,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => action(key, target),
            Self::App(_) => self.call(
                name,
                cu::wire::KeyName {
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
        action: fn(&str, &cu::AxQuery) -> cu::Result<cu::Ack>,
        window_id: &str,
        query: &cu::AxQuery,
    ) -> cu::Result<cu::Ack> {
        match self {
            Self::Local => action(window_id, query),
            Self::App(_) => self.call(
                name,
                cu::wire::AxAction {
                    window_id: window_id.to_string(),
                    query: query.clone(),
                },
            ),
        }
    }

    fn call<A: Serialize, T: DeserializeOwned>(&self, name: &str, args: A) -> cu::Result<T> {
        let Self::App(transport) = self else {
            unreachable!("call is only reached on the App backend");
        };
        let params =
            serde_json::to_value(args).map_err(|error| cu::Error::Usage(error.to_string()))?;
        let params = (!params.is_null()).then_some(params);
        let result = transport
            .request(name, params)
            .map_err(|error| decode_failure(&error))?;
        serde_json::from_value(result.unwrap_or(Value::Null)).map_err(|error| {
            cu::Error::Failed(format!("{name} returned an unreadable answer: {error}"))
        })
    }
}
