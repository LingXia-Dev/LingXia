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

    #[swift_bridge(swift_repr = "struct")]
    pub struct SwiftImageInfoResult {
        pub success: bool,
        pub error: String,
        pub width: u32,
        pub height: u32,
        pub mime_type: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct SwiftCompressImageResult {
        pub success: bool,
        pub error: String,
        pub path: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct SwiftVideoInfoResult {
        pub success: bool,
        pub error: String,
        pub width: u32,
        pub height: u32,
        pub duration_ms: u64,
        pub rotation: i32,
        pub has_rotation: bool,
        pub bitrate: u64,
        pub has_bitrate: bool,
        pub fps: f32,
        pub has_fps: bool,
        pub mime_type: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct SwiftVideoThumbnailResult {
        pub success: bool,
        pub error: String,
        pub path: String,
        pub width: u32,
        pub height: u32,
        pub mime_type: String,
    }

    #[swift_bridge(swift_repr = "struct")]
    pub struct SwiftCompressVideoResult {
        pub success: bool,
        pub error: String,
        pub path: String,
        pub width: u32,
        pub height: u32,
        pub duration_ms: u64,
        pub size: u64,
        pub mime_type: String,
    }

    extern "Swift" {
        // LxApp navigation functions
        #[swift_bridge(swift_name = "LxApp.openLxApp")]
        fn open_lxapp(
            appid: &str,
            path: &str,
            session_id: u64,
            presentation: i32,
            panel_id: &str,
        ) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeLxApp")]
        fn close_lxapp(appid: &str, session_id: u64) -> bool;

        #[swift_bridge(swift_name = "LxApp.navigate")]
        fn navigate(appid: &str, path: &str, animation_type: i32) -> bool;

        // TabBar UI update callback
        #[swift_bridge(swift_name = "LxApp.updateTabBarUI")]
        fn update_tabbar_ui(appid: &str) -> bool;

        // NavigationBar UI update callback
        #[swift_bridge(swift_name = "LxApp.updateNavBarUI")]
        fn update_navbar_ui(appid: &str) -> bool;

        // Orientation UI update callback
        #[swift_bridge(swift_name = "LxApp.updateOrientationUI")]
        fn update_orientation_ui(appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.openUrl")]
        fn open_url(owner_appid: &str, owner_session_id: u64, url: &str, target: i32) -> bool;

        #[swift_bridge(swift_name = "LxApp.exitApp")]
        fn exit_app() -> bool;

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

        #[swift_bridge(swift_name = "LxApp.presentSurface")]
        fn present_surface(
            id: &str,
            appid: &str,
            path: &str,
            session_id: u64,
            page_instance_id: &str,
            content: i32,
            kind: i32,
            width: f64,
            height: f64,
            width_ratio: f64,
            height_ratio: f64,
            position: i32,
        ) -> bool;

        #[swift_bridge(swift_name = "LxApp.closeSurface")]
        fn close_surface(id: &str, appid: &str, reason: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.showSurface")]
        fn show_surface(id: &str, appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.hideSurface")]
        fn hide_surface(id: &str, appid: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.previewMedia")]
        fn preview_media(items_json: &str, callback_id: u64) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.cancelPreview")]
        fn cancel_preview_media(callback_id: u64) -> bool;

        #[swift_bridge(swift_name = "LxApp.reviewDocument")]
        fn review_document(file_path: &str, mime_type: &str, show_menu: bool) -> bool;

        #[swift_bridge(swift_name = "LxApp.openDocumentExternal")]
        fn open_document_external(file_path: &str, mime_type: &str, show_menu: bool) -> bool;

        #[swift_bridge(swift_name = "LxApp.chooseFile")]
        fn choose_file(
            title: &str,
            default_path: &str,
            multiple: bool,
            filters_json: &str,
            callback_id: u64,
        ) -> bool;

        #[swift_bridge(swift_name = "LxApp.chooseDirectory")]
        fn choose_directory(title: &str, default_path: &str, callback_id: u64) -> bool;

        #[swift_bridge(swift_name = "LxApp.revealInFileManager")]
        fn reveal_in_file_manager(path: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.chooseMedia")]
        fn choose_media(
            max_count: u32,
            mode: &str,
            source_types_json: &str,
            camera_facing: &str,
            max_duration: Option<u32>,
            callback_id: u64,
        ) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.scanCode")]
        fn scan_code(scan_types_json: &str, only_from_camera: bool, callback_id: u64) -> bool;

        // Copy album media URI to destination file path with normalization.
        // media_type: 0=image(JPG), 1=video(MP4)
        #[swift_bridge(swift_name = "LxAppMedia.copyAlbumMediaToFile")]
        fn copy_album_media_to_file(uri: &str, destination_path: &str, media_type: i32) -> bool;

        #[swift_bridge(swift_name = "LxAppMedia.getImageInfo")]
        fn get_image_info(uri: &str) -> SwiftImageInfoResult;

        #[swift_bridge(swift_name = "LxAppMedia.compressImage")]
        fn compress_image(
            source_uri: &str,
            quality: i32,
            target_width: i32,
            target_height: i32,
            output_path: &str,
        ) -> SwiftCompressImageResult;

        #[swift_bridge(swift_name = "LxAppMedia.getVideoInfo")]
        fn get_video_info(uri: &str) -> SwiftVideoInfoResult;

        #[swift_bridge(swift_name = "LxAppMedia.extractVideoThumbnail")]
        fn extract_video_thumbnail(
            source_uri: &str,
            quality: i32,
            target_width: i32,
            target_height: i32,
            time_ms: i64,
            output_path: &str,
        ) -> SwiftVideoThumbnailResult;

        #[swift_bridge(swift_name = "LxAppMedia.compressVideo")]
        fn compress_video(
            source_uri: &str,
            quality: Option<&str>,
            bitrate_kbps: u32,
            fps: u32,
            resolution_ratio: f32,
            output_path: &str,
        ) -> SwiftCompressVideoResult;

        // Video player control (native component-backed)
        // Note: UI manages component lifecycle; Rust only dispatches commands.
        #[swift_bridge(swift_name = "LxAppVideo.setVideoPlayerCallback")]
        fn set_video_player_callback(component_id: &str, callback_id: u64) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.dispatchVideoCommand")]
        fn dispatch_video_command(component_id: &str, name: &str, params_json: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.createStreamDecoder")]
        fn create_stream_decoder(component_id: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.configureStreamVideo")]
        fn configure_stream_video(component_id: &str, config_json: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.configureStreamAudio")]
        fn configure_stream_audio(component_id: &str, config_json: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.pushStreamVideo")]
        fn push_stream_video(
            component_id: &str,
            data: Vec<u8>,
            dts_ms: u32,
            pts_ms: u32,
            keyframe: bool,
        ) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.pushStreamAudio")]
        fn push_stream_audio(component_id: &str, data: Vec<u8>, dts_ms: u32, pts_ms: u32) -> bool;

        #[swift_bridge(swift_name = "LxAppVideo.stopStreamDecoder")]
        fn stop_stream_decoder(component_id: &str) -> bool;

        // Pull-to-refresh functions
        #[swift_bridge(swift_name = "LxApp.startPullDownRefresh")]
        fn start_pull_down_refresh(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxApp.stopPullDownRefresh")]
        fn stop_pull_down_refresh(appid: &str, path: &str) -> bool;

        #[swift_bridge(swift_name = "LxAppCapsuleButtons.getCapsuleRect")]
        fn get_capsule_rect(callback_id: u64);

        // WiFi APIs
        #[swift_bridge(swift_name = "LxAppWifi.startWifi")]
        fn start_wifi(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppWifi.stopWifi")]
        fn stop_wifi(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppWifi.connectWifi")]
        fn connect_wifi(callback_id: u64, ssid: &str, password: Option<&str>);

        #[swift_bridge(swift_name = "LxAppWifi.getWifiList")]
        fn get_wifi_list(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppWifi.getConnectedWifi")]
        fn get_connected_wifi(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppWifi.isWifiEnabled")]
        fn is_wifi_enabled() -> bool;

        #[swift_bridge(swift_name = "LxAppWifi.addWifiStateListener")]
        fn add_wifi_state_listener(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppWifi.removeWifiStateListener")]
        fn remove_wifi_state_listener(callback_id: u64);

        // Network APIs
        #[swift_bridge(swift_name = "LxAppNetwork.getNetworkInfo")]
        fn get_network_info(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppNetwork.addNetworkChangeListener")]
        fn add_network_change_listener(callback_id: u64);

        #[swift_bridge(swift_name = "LxAppNetwork.removeNetworkChangeListener")]
        fn remove_network_change_listener(callback_id: u64);
    }
}

// Re-export the bridge functions for use in other modules
#[cfg(target_os = "macos")]
pub use bridge::reveal_in_file_manager;
pub use bridge::{
    ActionSheetOptions, ModalOptions, ToastIcon, ToastOptions, ToastPosition, cancel_preview_media,
    close_lxapp, close_surface, exit_app, hide_surface, hide_toast, navigate,
    open_document_external, open_lxapp, open_url, present_surface, preview_media, review_document,
    show_action_sheet, show_modal, show_surface, show_toast, update_navbar_ui,
    update_orientation_ui, update_tabbar_ui,
};

#[cfg(target_os = "ios")]
pub use bridge::{choose_directory, choose_file};
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unused_imports)]
pub use bridge::{
    compress_image, compress_video, configure_stream_audio, configure_stream_video,
    copy_album_media_to_file, create_stream_decoder, extract_video_thumbnail, get_capsule_rect,
    get_image_info, get_video_info, push_stream_audio, push_stream_video, scan_code,
    stop_stream_decoder,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unused_imports)]
pub use bridge::{dispatch_video_command, set_video_player_callback};

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unused_imports)]
pub use bridge::{choose_media, start_pull_down_refresh, stop_pull_down_refresh};

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unused_imports)]
pub use bridge::{
    add_wifi_state_listener, connect_wifi, get_connected_wifi, get_wifi_list, is_wifi_enabled,
    remove_wifi_state_listener, start_wifi, stop_wifi,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[allow(unused_imports)]
pub use bridge::{add_network_change_listener, get_network_info, remove_network_change_listener};
