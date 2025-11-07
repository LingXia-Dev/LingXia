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
        pub item_color: String,
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
        pub cancel_text: String,
        pub cancel_button_color: String,
        pub cancel_text_color: String,
        pub confirm_text: String,
        pub confirm_button_color: String,
        pub confirm_text_color: String,
    }

    pub enum PopupPositionBridge {
        Center,
        Bottom,
        Left,
        Right,
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

        #[swift_bridge(swift_name = "LxApp.showPopup")]
        fn show_popup(
            appid: &str,
            path: &str,
            width_ratio: f64,
            height_ratio: f64,
            position: PopupPositionBridge,
        ) -> bool;

        #[swift_bridge(swift_name = "LxApp.hidePopup")]
        fn hide_popup(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.previewMedia")]
        fn preview_media(items_json: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.openDocument")]
        fn open_document(file_path: &str, mime_type: &str, show_menu: bool) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.chooseMedia")]
        fn choose_media(
            max_count: u32,
            mode: &str,
            source_types_json: &str,
            camera_facing: &str,
            max_duration: &str,
            callback_id: u64,
        ) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.scanCode")]
        fn scan_code(scan_types_json: &str, only_from_camera: bool, callback_id: u64) -> bool;

        // Copy album media URI to destination file path with normalization.
        // media_type: 0=image(JPG), 1=video(MP4)
        #[swift_bridge(swift_name = "LxAppMedia.copyAlbumMediaToFile")]
        fn copy_album_media_to_file(uri: &str, destination_path: &str, media_type: i32) -> bool;
    }
}

// Re-export the bridge functions for use in other modules
pub use bridge::{
    ActionSheetOptions, ModalOptions, PickerOptions, PopupPositionBridge, ToastIcon, ToastOptions,
    ToastPosition, choose_media, close_lxapp, copy_album_media_to_file, hide_popup, hide_toast,
    launch_with_url, navigate, open_document, open_lxapp, preview_media, scan_code,
    show_action_sheet, show_modal, show_picker, show_popup, show_toast, update_navbar_ui,
    update_tabbar_ui,
};
