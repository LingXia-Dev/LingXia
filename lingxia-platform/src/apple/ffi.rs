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
    }

    // ActionSheet configuration for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct ActionSheetOptions {
        pub options: Vec<String>,
        pub cancel_text: String,
    }

    // Modal result for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct ModalResult {
        pub confirm: bool,
        pub cancel: bool,
    }

    // Picker configuration for Swift
    #[swift_bridge(swift_repr = "struct")]
    pub struct PickerOptions {
        pub columns_json: String,
        pub text_color: String,
        pub cancel_text: String,
        pub cancel_color: String,
        pub confirm_text: String,
        pub confirm_color: String,
        pub confirm_text_color: String,
    }

    extern "Swift" {
        // LxApp navigation functions
        #[swift_bridge(swift_name = "LxApp.openLxApp")]
        fn open_lxapp(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeLxApp")]
        fn close_lxapp(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.navigate")]
        fn navigate(appid: &str, path: &str, animation_type: i32) -> bool;

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

        // Modal functions (synchronous with callback)
        #[swift_bridge(swift_name = "LxApp.showModal")]
        fn show_modal(options: ModalOptions, callback_id: u64);

        // Action sheet functions (synchronous with callback)
        #[swift_bridge(swift_name = "LxApp.showActionSheet")]
        fn show_action_sheet(options: ActionSheetOptions, callback_id: u64);

        // Picker functions (synchronous with callback)
        #[swift_bridge(swift_name = "LxApp.showPicker")]
        fn show_picker(options: PickerOptions, callback_id: u64);
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{
    ActionSheetOptions, ModalOptions, PickerOptions, ToastIcon, ToastOptions, ToastPosition,
    close_lxapp, hide_toast, launch_with_url, navigate, open_lxapp, show_action_sheet, show_modal,
    show_picker, show_toast, update_navbar_ui, update_tabbar_ui,
};
