//! The OHOS NDK surface this adapter uses, declared by hand: no published crate
//! covers the ArkUI node, XComponent surface holder or accessibility provider APIs.
//! Enum values follow the API 24 NDK headers. The floor is API 20 (XComponent node
//! surface holder, frame callback, accessibility provider); symbols newer than the
//! app's compatible API level are resolved at run time instead (see [`optional`]).
#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]
use std::ffi::{c_char, c_void};

macro_rules! opaque {
    ($($name:ident),* $(,)?) => {
        $(#[repr(C)] pub struct $name { _private: [u8; 0] })*
    };
}
opaque!(
    ArkUI_Node,
    ArkUI_NodeEvent,
    ArkUI_UIInputEvent,
    ArkUI_NodeContent,
    OH_ArkUI_SurfaceHolder,
    OH_ArkUI_SurfaceCallback,
    OHNativeWindow,
    OHNativeWindowBuffer,
    ArkUI_AccessibilityProvider,
    ArkUI_AccessibilityElementInfo,
    ArkUI_AccessibilityElementInfoList,
    ArkUI_AccessibilityEventInfo,
    ArkUI_AccessibilityActionArguments,
    InputMethod_TextEditorProxy,
    InputMethod_InputMethodProxy,
    InputMethod_AttachOptions,
    InputMethod_TextConfig,
    InputMethod_CursorInfo,
    InputMethod_PrivateCommand,
);

pub type ArkUI_NodeHandle = *mut ArkUI_Node;
pub type ArkUI_NodeContentHandle = *mut ArkUI_NodeContent;

// ---- ArkUI native node API ------------------------------------------------------------------

pub const ARKUI_NATIVE_NODE: i32 = 0;
pub const ARKUI_NODE_XCOMPONENT: i32 = 12;
pub const ARKUI_XCOMPONENT_TYPE_SURFACE: i32 = 0;

pub const NODE_ID: i32 = 5;
pub const NODE_FOCUSABLE: i32 = 39;
pub const NODE_FOCUS_STATUS: i32 = 66;
pub const NODE_WIDTH_PERCENT: i32 = 71;
pub const NODE_HEIGHT_PERCENT: i32 = 72;
pub const NODE_XCOMPONENT_ID: i32 = 12000;
pub const NODE_XCOMPONENT_TYPE: i32 = 12001;

pub const NODE_TOUCH_EVENT: i32 = 0;
pub const NODE_ON_FOCUS: i32 = 4;
pub const NODE_ON_BLUR: i32 = 5;
pub const NODE_ON_HOVER: i32 = 9;
pub const NODE_ON_MOUSE: i32 = 10;
pub const NODE_ON_KEY_EVENT: i32 = 21;
pub const NODE_ON_AXIS: i32 = 25;

pub const UI_TOUCH_EVENT_ACTION_CANCEL: i32 = 0;
pub const UI_TOUCH_EVENT_ACTION_DOWN: i32 = 1;
pub const UI_TOUCH_EVENT_ACTION_MOVE: i32 = 2;
pub const UI_TOUCH_EVENT_ACTION_UP: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub union ArkUI_NumberValue {
    pub f32: f32,
    pub i32: i32,
    pub u32: u32,
}

#[repr(C)]
pub struct ArkUI_AttributeItem {
    pub value: *const ArkUI_NumberValue,
    pub size: i32,
    pub string: *const c_char,
    pub object: *mut c_void,
}

type EventReceiver = unsafe extern "C" fn(event: *mut ArkUI_NodeEvent);

/// `ArkUI_NativeNodeAPI_1`, declared up to the last member this adapter calls.
#[repr(C)]
pub struct ArkUI_NativeNodeAPI_1 {
    pub version: i32,
    pub create_node: unsafe extern "C" fn(kind: i32) -> ArkUI_NodeHandle,
    pub dispose_node: unsafe extern "C" fn(node: ArkUI_NodeHandle),
    pub add_child: unsafe extern "C" fn(parent: ArkUI_NodeHandle, child: ArkUI_NodeHandle) -> i32,
    pub remove_child:
        unsafe extern "C" fn(parent: ArkUI_NodeHandle, child: ArkUI_NodeHandle) -> i32,
    pub insert_child_after: *const c_void,
    pub insert_child_before: *const c_void,
    pub insert_child_at: *const c_void,
    pub set_attribute: unsafe extern "C" fn(
        node: ArkUI_NodeHandle,
        attribute: i32,
        item: *const ArkUI_AttributeItem,
    ) -> i32,
    pub get_attribute:
        unsafe extern "C" fn(node: ArkUI_NodeHandle, attribute: i32) -> *const ArkUI_AttributeItem,
    pub reset_attribute: *const c_void,
    pub register_node_event: unsafe extern "C" fn(
        node: ArkUI_NodeHandle,
        kind: i32,
        target_id: i32,
        user_data: *mut c_void,
    ) -> i32,
    pub unregister_node_event: unsafe extern "C" fn(node: ArkUI_NodeHandle, kind: i32),
    pub register_node_event_receiver: *const c_void,
    pub unregister_node_event_receiver: *const c_void,
    pub mark_dirty: *const c_void,
    pub get_total_child_count: *const c_void,
    pub get_child_at: *const c_void,
    pub get_first_child: *const c_void,
    pub get_last_child: *const c_void,
    pub get_previous_sibling: *const c_void,
    pub get_next_sibling: *const c_void,
    pub register_node_custom_event: *const c_void,
    pub unregister_node_custom_event: *const c_void,
    pub register_node_custom_event_receiver: *const c_void,
    pub unregister_node_custom_event_receiver: *const c_void,
    pub set_measured_size: *const c_void,
    pub set_layout_position: *const c_void,
    pub get_measured_size: *const c_void,
    pub get_layout_position: *const c_void,
    pub measure_node: *const c_void,
    pub layout_node: *const c_void,
    pub add_node_event_receiver:
        unsafe extern "C" fn(node: ArkUI_NodeHandle, receiver: EventReceiver) -> i32,
    pub remove_node_event_receiver:
        unsafe extern "C" fn(node: ArkUI_NodeHandle, receiver: EventReceiver) -> i32,
}

#[link(name = "ace_ndk.z")]
unsafe extern "C" {
    pub fn OH_ArkUI_QueryModuleInterfaceByName(kind: i32, name: *const c_char) -> *mut c_void;
    pub fn OH_ArkUI_GetNodeContentFromNapiValue(
        env: napi_sys_ohos::napi_env,
        value: napi_sys_ohos::napi_value,
        content: *mut ArkUI_NodeContentHandle,
    ) -> i32;
    pub fn OH_ArkUI_NodeContent_AddNode(
        content: ArkUI_NodeContentHandle,
        node: ArkUI_NodeHandle,
    ) -> i32;
    pub fn OH_ArkUI_NodeContent_RemoveNode(
        content: ArkUI_NodeContentHandle,
        node: ArkUI_NodeHandle,
    ) -> i32;

    pub fn OH_ArkUI_NodeEvent_GetEventType(event: *mut ArkUI_NodeEvent) -> i32;
    pub fn OH_ArkUI_NodeEvent_GetUserData(event: *mut ArkUI_NodeEvent) -> *mut c_void;
    pub fn OH_ArkUI_NodeEvent_GetInputEvent(event: *mut ArkUI_NodeEvent)
    -> *mut ArkUI_UIInputEvent;
    pub fn OH_ArkUI_UIInputEvent_GetAction(event: *const ArkUI_UIInputEvent) -> i32;
    pub fn OH_ArkUI_UIInputEvent_GetEventTime(event: *const ArkUI_UIInputEvent) -> i64;
    pub fn OH_ArkUI_PointerEvent_GetX(event: *const ArkUI_UIInputEvent) -> f32;
    pub fn OH_ArkUI_PointerEvent_GetY(event: *const ArkUI_UIInputEvent) -> f32;
    pub fn OH_ArkUI_PointerEvent_GetHistorySize(event: *const ArkUI_UIInputEvent) -> u32;
    pub fn OH_ArkUI_PointerEvent_GetHistoryX(
        event: *const ArkUI_UIInputEvent,
        pointer: u32,
        history: u32,
    ) -> f32;
    pub fn OH_ArkUI_PointerEvent_GetHistoryY(
        event: *const ArkUI_UIInputEvent,
        pointer: u32,
        history: u32,
    ) -> f32;
    pub fn OH_ArkUI_PointerEvent_GetHistoryEventTime(
        event: *const ArkUI_UIInputEvent,
        history: u32,
    ) -> i64;
    pub fn OH_ArkUI_PointerEvent_SetStopPropagation(
        event: *const ArkUI_UIInputEvent,
        stop: bool,
    ) -> i32;

    // XComponent surface holder (API 19); frame callback, soft keyboard and provider (API 20).
    pub fn OH_ArkUI_SurfaceHolder_Create(node: ArkUI_NodeHandle) -> *mut OH_ArkUI_SurfaceHolder;
    pub fn OH_ArkUI_SurfaceHolder_Dispose(holder: *mut OH_ArkUI_SurfaceHolder);
    pub fn OH_ArkUI_SurfaceHolder_SetUserData(
        holder: *mut OH_ArkUI_SurfaceHolder,
        data: *mut c_void,
    ) -> i32;
    pub fn OH_ArkUI_SurfaceHolder_GetUserData(holder: *mut OH_ArkUI_SurfaceHolder) -> *mut c_void;
    pub fn OH_ArkUI_SurfaceHolder_AddSurfaceCallback(
        holder: *mut OH_ArkUI_SurfaceHolder,
        callback: *mut OH_ArkUI_SurfaceCallback,
    ) -> i32;
    pub fn OH_ArkUI_SurfaceCallback_Create() -> *mut OH_ArkUI_SurfaceCallback;
    pub fn OH_ArkUI_SurfaceCallback_Dispose(callback: *mut OH_ArkUI_SurfaceCallback);
    pub fn OH_ArkUI_SurfaceCallback_SetSurfaceCreatedEvent(
        callback: *mut OH_ArkUI_SurfaceCallback,
        event: unsafe extern "C" fn(holder: *mut OH_ArkUI_SurfaceHolder),
    );
    pub fn OH_ArkUI_SurfaceCallback_SetSurfaceChangedEvent(
        callback: *mut OH_ArkUI_SurfaceCallback,
        event: unsafe extern "C" fn(holder: *mut OH_ArkUI_SurfaceHolder, width: u64, height: u64),
    );
    pub fn OH_ArkUI_SurfaceCallback_SetSurfaceDestroyedEvent(
        callback: *mut OH_ArkUI_SurfaceCallback,
        event: unsafe extern "C" fn(holder: *mut OH_ArkUI_SurfaceHolder),
    );
    pub fn OH_ArkUI_XComponent_GetNativeWindow(
        holder: *mut OH_ArkUI_SurfaceHolder,
    ) -> *mut OHNativeWindow;
    pub fn OH_ArkUI_XComponent_RegisterOnFrameCallback(
        node: ArkUI_NodeHandle,
        callback: unsafe extern "C" fn(node: ArkUI_NodeHandle, timestamp: u64, target: u64),
    ) -> i32;
    pub fn OH_ArkUI_XComponent_UnregisterOnFrameCallback(node: ArkUI_NodeHandle) -> i32;
    pub fn OH_ArkUI_XComponent_SetNeedSoftKeyboard(node: ArkUI_NodeHandle, need: bool) -> i32;
    pub fn OH_ArkUI_AccessibilityProvider_Create(
        node: ArkUI_NodeHandle,
    ) -> *mut ArkUI_AccessibilityProvider;
    pub fn OH_ArkUI_AccessibilityProvider_Dispose(provider: *mut ArkUI_AccessibilityProvider);

    // Accessibility provider (API 13; the per-instance callbacks API 15).
    pub fn OH_ArkUI_AccessibilityProviderRegisterCallbackWithInstance(
        instance: *const c_char,
        provider: *mut ArkUI_AccessibilityProvider,
        callbacks: *mut ArkUI_AccessibilityProviderCallbacksWithInstance,
    ) -> i32;
    pub fn OH_ArkUI_SendAccessibilityAsyncEvent(
        provider: *mut ArkUI_AccessibilityProvider,
        event: *mut ArkUI_AccessibilityEventInfo,
        callback: Option<unsafe extern "C" fn(code: i32)>,
    );
    pub fn OH_ArkUI_AddAndGetAccessibilityElementInfo(
        list: *mut ArkUI_AccessibilityElementInfoList,
    ) -> *mut ArkUI_AccessibilityElementInfo;
    pub fn OH_ArkUI_AccessibilityElementInfoSetElementId(
        info: *mut ArkUI_AccessibilityElementInfo,
        id: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetParentId(
        info: *mut ArkUI_AccessibilityElementInfo,
        id: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetComponentType(
        info: *mut ArkUI_AccessibilityElementInfo,
        kind: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetContents(
        info: *mut ArkUI_AccessibilityElementInfo,
        contents: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetHintText(
        info: *mut ArkUI_AccessibilityElementInfo,
        hint: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetAccessibilityText(
        info: *mut ArkUI_AccessibilityElementInfo,
        text: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetChildNodeIds(
        info: *mut ArkUI_AccessibilityElementInfo,
        count: i32,
        ids: *mut i64,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetOperationActions(
        info: *mut ArkUI_AccessibilityElementInfo,
        count: i32,
        actions: *mut ArkUI_AccessibleAction,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetScreenRect(
        info: *mut ArkUI_AccessibilityElementInfo,
        rect: *mut ArkUI_AccessibleRect,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetCheckable(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetChecked(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetFocusable(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetFocused(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetVisible(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetAccessibilityFocused(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetSelected(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetClickable(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetEnabled(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetIsPassword(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetScrollable(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetEditable(
        info: *mut ArkUI_AccessibilityElementInfo,
        value: bool,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetRangeInfo(
        info: *mut ArkUI_AccessibilityElementInfo,
        range: *mut ArkUI_AccessibleRangeInfo,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetSelectedTextStart(
        info: *mut ArkUI_AccessibilityElementInfo,
        start: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetSelectedTextEnd(
        info: *mut ArkUI_AccessibilityElementInfo,
        end: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetItemCount(
        info: *mut ArkUI_AccessibilityElementInfo,
        count: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetCurrentItemIndex(
        info: *mut ArkUI_AccessibilityElementInfo,
        index: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityElementInfoSetAccessibilityLevel(
        info: *mut ArkUI_AccessibilityElementInfo,
        level: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_CreateAccessibilityEventInfo() -> *mut ArkUI_AccessibilityEventInfo;
    pub fn OH_ArkUI_DestoryAccessibilityEventInfo(event: *mut ArkUI_AccessibilityEventInfo);
    pub fn OH_ArkUI_AccessibilityEventSetEventType(
        event: *mut ArkUI_AccessibilityEventInfo,
        kind: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityEventSetTextAnnouncedForAccessibility(
        event: *mut ArkUI_AccessibilityEventInfo,
        text: *const c_char,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityEventSetRequestFocusId(
        event: *mut ArkUI_AccessibilityEventInfo,
        id: i32,
    ) -> i32;
    pub fn OH_ArkUI_AccessibilityEventSetElementInfo(
        event: *mut ArkUI_AccessibilityEventInfo,
        info: *mut ArkUI_AccessibilityElementInfo,
    ) -> i32;
    pub fn OH_ArkUI_FindAccessibilityActionArgumentByKey(
        arguments: *mut ArkUI_AccessibilityActionArguments,
        key: *const c_char,
        value: *mut *mut c_char,
    ) -> i32;
    pub fn OH_ArkUI_CreateAccessibilityElementInfo() -> *mut ArkUI_AccessibilityElementInfo;
    pub fn OH_ArkUI_DestoryAccessibilityElementInfo(info: *mut ArkUI_AccessibilityElementInfo);
}

// ---- Accessibility types --------------------------------------------------------------------

pub const ACTION_CLICK: u32 = 0x10;
pub const ACTION_LONG_CLICK: u32 = 0x20;
pub const ACTION_GAIN_ACCESSIBILITY_FOCUS: u32 = 0x40;
pub const ACTION_CLEAR_ACCESSIBILITY_FOCUS: u32 = 0x80;
pub const ACTION_SCROLL_FORWARD: u32 = 0x100;
pub const ACTION_SCROLL_BACKWARD: u32 = 0x200;
pub const ACTION_COPY: u32 = 0x400;
pub const ACTION_PASTE: u32 = 0x800;
pub const ACTION_CUT: u32 = 0x1000;
pub const ACTION_SELECT_TEXT: u32 = 0x2000;
pub const ACTION_SET_TEXT: u32 = 0x4000;
pub const ACTION_SET_CURSOR_POSITION: u32 = 0x100000;

pub const EVENT_CLICKED: i32 = 0x1;
pub const EVENT_SELECTED: i32 = 0x4;
pub const EVENT_TEXT_UPDATE: i32 = 0x10;
pub const EVENT_PAGE_CONTENT_UPDATE: i32 = 0x800;
pub const EVENT_SCROLLED: i32 = 0x1000;
pub const EVENT_ACCESSIBILITY_FOCUSED: i32 = 0x8000;
pub const EVENT_ACCESSIBILITY_FOCUS_CLEARED: i32 = 0x10000;
pub const EVENT_REQUEST_ACCESSIBILITY_FOCUS: i32 = 0x2000000;
pub const EVENT_ANNOUNCE_FOR_ACCESSIBILITY: i32 = 0x10000000;
pub const EVENT_FOCUS_NODE_UPDATE: i32 = 0x10000001;

pub const SEARCH_PREFETCH_PREDECESSORS: i32 = 1 << 0;
pub const SEARCH_PREFETCH_SIBLINGS: i32 = 1 << 1;
pub const SEARCH_PREFETCH_CHILDREN: i32 = 1 << 2;
pub const SEARCH_PREFETCH_RECURSIVE_CHILDREN: i32 = 1 << 3;

pub const FOCUS_TYPE_INPUT: i32 = 1 << 0;
pub const FOCUS_TYPE_ACCESSIBILITY: i32 = 1 << 1;

pub const RESULT_SUCCESSFUL: i32 = 0;
pub const RESULT_FAILED: i32 = -1;

#[repr(C)]
pub struct ArkUI_AccessibleAction {
    pub kind: u32,
    pub description: *const c_char,
}

#[repr(C)]
pub struct ArkUI_AccessibleRect {
    pub left_top_x: i32,
    pub left_top_y: i32,
    pub right_bottom_x: i32,
    pub right_bottom_y: i32,
}

#[repr(C)]
pub struct ArkUI_AccessibleRangeInfo {
    pub min: f64,
    pub max: f64,
    pub current: f64,
}

type Instance = *const c_char;
type ElementList = *mut ArkUI_AccessibilityElementInfoList;

#[repr(C)]
pub struct ArkUI_AccessibilityProviderCallbacksWithInstance {
    pub find_by_id: unsafe extern "C" fn(Instance, i64, i32, i32, ElementList) -> i32,
    pub find_by_text: unsafe extern "C" fn(Instance, i64, *const c_char, i32, ElementList) -> i32,
    pub find_focused:
        unsafe extern "C" fn(Instance, i64, i32, i32, *mut ArkUI_AccessibilityElementInfo) -> i32,
    pub find_next_focus:
        unsafe extern "C" fn(Instance, i64, i32, i32, *mut ArkUI_AccessibilityElementInfo) -> i32,
    pub execute_action: unsafe extern "C" fn(
        Instance,
        i64,
        u32,
        *mut ArkUI_AccessibilityActionArguments,
        i32,
    ) -> i32,
    pub clear_focused: unsafe extern "C" fn(Instance) -> i32,
    pub cursor_position: unsafe extern "C" fn(Instance, i64, i32, *mut i32) -> i32,
}

// ---- Native window --------------------------------------------------------------------------

#[link(name = "native_window")]
unsafe extern "C" {
    pub fn OH_NativeWindow_NativeObjectReference(object: *mut c_void) -> i32;
    pub fn OH_NativeWindow_NativeObjectUnreference(object: *mut c_void) -> i32;
    pub fn OH_NativeWindow_NativeWindowHandleOpt(
        window: *mut OHNativeWindow,
        code: i32,
        ...
    ) -> i32;
    pub fn OH_NativeWindow_NativeWindowRequestBuffer(
        window: *mut OHNativeWindow,
        buffer: *mut *mut OHNativeWindowBuffer,
        fence: *mut i32,
    ) -> i32;
    pub fn OH_NativeWindow_NativeWindowAbortBuffer(
        window: *mut OHNativeWindow,
        buffer: *mut OHNativeWindowBuffer,
    ) -> i32;
}

/// `NativeWindowOperation` codes.
pub const SET_BUFFER_GEOMETRY: i32 = 0;
pub const SET_FORMAT: i32 = 3;
pub const SET_USAGE: i32 = 4;

// ---- Input method ---------------------------------------------------------------------------

#[link(name = "ohinputmethod")]
unsafe extern "C" {
    pub fn OH_TextEditorProxy_Create() -> *mut InputMethod_TextEditorProxy;
    pub fn OH_TextEditorProxy_Destroy(proxy: *mut InputMethod_TextEditorProxy);
    pub fn OH_AttachOptions_Create(show_keyboard: bool) -> *mut InputMethod_AttachOptions;
    pub fn OH_AttachOptions_Destroy(options: *mut InputMethod_AttachOptions);
    pub fn OH_InputMethodController_Attach(
        editor: *mut InputMethod_TextEditorProxy,
        options: *mut InputMethod_AttachOptions,
        proxy: *mut *mut InputMethod_InputMethodProxy,
    ) -> i32;
    pub fn OH_InputMethodController_Detach(proxy: *mut InputMethod_InputMethodProxy) -> i32;
    pub fn OH_InputMethodProxy_ShowKeyboard(proxy: *mut InputMethod_InputMethodProxy) -> i32;
    pub fn OH_InputMethodProxy_HideKeyboard(proxy: *mut InputMethod_InputMethodProxy) -> i32;
    pub fn OH_InputMethodProxy_NotifySelectionChange(
        proxy: *mut InputMethod_InputMethodProxy,
        text: *mut u16,
        length: usize,
        start: i32,
        end: i32,
    ) -> i32;
    pub fn OH_InputMethodProxy_NotifyConfigurationChange(
        proxy: *mut InputMethod_InputMethodProxy,
        enter_key: i32,
        text_type: i32,
    ) -> i32;
    pub fn OH_InputMethodProxy_NotifyCursorUpdate(
        proxy: *mut InputMethod_InputMethodProxy,
        cursor: *mut InputMethod_CursorInfo,
    ) -> i32;
    pub fn OH_TextConfig_SetInputType(config: *mut InputMethod_TextConfig, kind: i32) -> i32;
    pub fn OH_TextConfig_SetEnterKeyType(config: *mut InputMethod_TextConfig, kind: i32) -> i32;
    pub fn OH_TextConfig_SetPreviewTextSupport(
        config: *mut InputMethod_TextConfig,
        supported: bool,
    ) -> i32;
    pub fn OH_TextConfig_SetSelection(
        config: *mut InputMethod_TextConfig,
        start: i32,
        end: i32,
    ) -> i32;
    pub fn OH_TextConfig_SetWindowId(config: *mut InputMethod_TextConfig, window: i32) -> i32;
    pub fn OH_CursorInfo_Create(
        left: f64,
        top: f64,
        width: f64,
        height: f64,
    ) -> *mut InputMethod_CursorInfo;
    pub fn OH_CursorInfo_Destroy(cursor: *mut InputMethod_CursorInfo);
}

/// The `OH_TextEditorProxy_Set*Func` setters, one per editor callback.
macro_rules! editor_setters {
    ($($setter:ident($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[link(name = "ohinputmethod")]
        unsafe extern "C" {
            $(pub fn $setter(
                proxy: *mut InputMethod_TextEditorProxy,
                callback: unsafe extern "C" fn(*mut InputMethod_TextEditorProxy $(, $arg)*) $(-> $ret)?,
            ) -> i32;)*
        }
    };
}
editor_setters! {
    OH_TextEditorProxy_SetGetTextConfigFunc(*mut InputMethod_TextConfig);
    OH_TextEditorProxy_SetInsertTextFunc(*const u16, usize);
    OH_TextEditorProxy_SetDeleteForwardFunc(i32);
    OH_TextEditorProxy_SetDeleteBackwardFunc(i32);
    OH_TextEditorProxy_SetSendKeyboardStatusFunc(i32);
    OH_TextEditorProxy_SetSendEnterKeyFunc(i32);
    OH_TextEditorProxy_SetMoveCursorFunc(i32);
    OH_TextEditorProxy_SetHandleSetSelectionFunc(i32, i32);
    OH_TextEditorProxy_SetHandleExtendActionFunc(i32);
    OH_TextEditorProxy_SetGetLeftTextOfCursorFunc(i32, *mut u16, *mut usize);
    OH_TextEditorProxy_SetGetRightTextOfCursorFunc(i32, *mut u16, *mut usize);
    OH_TextEditorProxy_SetGetTextIndexAtCursorFunc() -> i32;
    OH_TextEditorProxy_SetReceivePrivateCommandFunc(*mut *mut InputMethod_PrivateCommand, usize) -> i32;
    OH_TextEditorProxy_SetSetPreviewTextFunc(*const u16, usize, i32, i32) -> i32;
    OH_TextEditorProxy_SetFinishTextPreviewFunc();
}

// ---- Fonts ----------------------------------------------------------------------------------

#[repr(C)]
pub struct OH_Drawing_FontFallbackInfo {
    pub language: *mut c_char,
    pub family_name: *mut c_char,
}

#[repr(C)]
pub struct OH_Drawing_FontFallbackGroup {
    pub group_name: *mut c_char,
    pub fallback_info_size: usize,
    pub fallback_info_set: *mut OH_Drawing_FontFallbackInfo,
}

#[repr(C)]
pub struct OH_Drawing_FontAliasInfo {
    pub family_name: *mut c_char,
    pub weight: i32,
}

#[repr(C)]
pub struct OH_Drawing_FontGenericInfo {
    pub family_name: *mut c_char,
    pub alias_info_size: usize,
    pub adjust_info_size: usize,
    pub alias_info_set: *mut OH_Drawing_FontAliasInfo,
    pub adjust_info_set: *mut c_void,
}

#[repr(C)]
pub struct OH_Drawing_FontConfigInfo {
    pub font_dir_size: usize,
    pub font_generic_info_size: usize,
    pub fallback_group_size: usize,
    pub font_dir_set: *mut *mut c_char,
    pub font_generic_info_set: *mut OH_Drawing_FontGenericInfo,
    pub fallback_group_set: *mut OH_Drawing_FontFallbackGroup,
}

#[link(name = "native_drawing")]
unsafe extern "C" {
    pub fn OH_Drawing_GetSystemFontConfigInfo(error: *mut i32) -> *mut OH_Drawing_FontConfigInfo;
    pub fn OH_Drawing_DestroySystemFontConfigInfo(info: *mut OH_Drawing_FontConfigInfo);
}

// ---- Symbols above the compatible API level ---------------------------------------------------

/// Looks up `name` in the already loaded ArkUI library; `None` on a system that lacks it.
pub fn optional(name: &std::ffi::CStr) -> Option<*mut c_void> {
    // SAFETY: RTLD_DEFAULT searches the loaded libraries; the name is NUL-terminated.
    let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    (!symbol.is_null()).then_some(symbol)
}
