#[swift_bridge::bridge]
mod bridge {
    // Toast icon types
    pub enum ToastIcon {
        Success,
        Error,
        Loading,
        None,
    }

    // Toast position types
    pub enum ToastPosition {
        Top,
        Center,
        Bottom,
    }

    // Toast configuration for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct ToastOptions {
        pub title: String,
        pub icon: ToastIcon,
        pub image: String,
        pub duration: f64,
        pub mask: bool,
        pub position: ToastPosition,
    }

    // Modal configuration for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct ModalOptions {
        pub title: String,
        pub content: String,
        pub show_cancel: bool,
        pub cancel_text: String,
        pub cancel_color: String,
        pub confirm_text: String,
        pub confirm_color: String,
        pub editable: bool,
        pub placeholder_text: String,
    }

    // Modal result for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct ModalResult {
        pub confirm: bool,
        pub cancel: bool,
        pub content: String, // User input content
    }

    extern "Swift" {
        // LxApp navigation functions
        #[swift_bridge(swift_name = "LxApp.openLxApp")]
        fn open_lxapp(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeLxApp")]
        fn close_lxapp(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.switchPage")]
        fn switch_page(appid: &str, path: &str) -> bool;

        // TabBar UI update callback
        #[swift_bridge(swift_name = "LxApp.updateTabBarUI")]
        fn update_tabbar_ui(appid: &str) -> bool;

        // NavigationBar UI update callback
        #[swift_bridge(swift_name = "LxApp.updateNavBarUI")]
        fn update_navbar_ui(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.launchWithUrl")]
        fn launch_with_url(url: &str);

        #[swift_bridge(swift_name = "LxApp.isPushEnabled")]
        fn is_push_enabled() -> bool;

        // Toast functions
        #[swift_bridge(swift_name = "LxApp.showToast")]
        fn show_toast(options: ToastOptions);

        #[swift_bridge(swift_name = "LxApp.hideToast")]
        fn hide_toast();

        // Modal functions (synchronous, blocks until user responds)
        #[swift_bridge(swift_name = "LxApp.showModal")]
        fn show_modal(options: ModalOptions) -> ModalResult;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{close_lxapp, launch_with_url, open_lxapp, switch_page};
