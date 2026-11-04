//! The XComponent's accessibility provider over the shared semantic projection:
//! ScreenReader and the UI test framework read the same nodes lxdev drives, so this
//! is where automation on HarmonyOS stands or falls. No role or state inference here.
use crate::{pages, sys};
use lingxia_view::interaction::view::{AccessibilityAction, AccessibilityEffect};
use lingxia_view::semantics::projection::{Action, Event, Role, SemanticNode};
use std::ffi::{CStr, CString, c_char};

/// The element id ArkUI asks for when it wants the provider's root.
const ROOT: i64 = -1;

/// ArkUI component types; the screen reader speaks the role from these.
fn component_type(role: Role) -> &'static CStr {
    match role {
        Role::Button | Role::MenuItem | Role::GridCell => c"Button",
        Role::CheckBox => c"Checkbox",
        Role::Switch => c"Toggle",
        Role::RadioGroup => c"Column",
        Role::RadioButton => c"Radio",
        Role::ComboBox => c"Select",
        Role::TextField => c"TextInput",
        Role::ScrollView | Role::HorizontalScrollView => c"Scroll",
        Role::Image => c"Image",
        Role::Slider => c"Slider",
        Role::List | Role::ListBox | Role::Menu => c"List",
        Role::ListItem | Role::Option => c"ListItem",
        Role::Grid => c"Grid",
        Role::Dialog => c"Dialog",
        Role::Text | Role::Link | Role::Heading | Role::Alert | Role::Status => c"Text",
    }
}

fn actions(node: &SemanticNode, focused: bool) -> Vec<u32> {
    let mut out = vec![if focused {
        sys::ACTION_CLEAR_ACCESSIBILITY_FOCUS
    } else {
        sys::ACTION_GAIN_ACCESSIBILITY_FOCUS
    }];
    for action in &node.actions {
        out.push(match action {
            Action::Click => sys::ACTION_CLICK,
            Action::ScrollForward => sys::ACTION_SCROLL_FORWARD,
            Action::ScrollBackward => sys::ACTION_SCROLL_BACKWARD,
            Action::SetText => sys::ACTION_SET_TEXT,
            Action::SetSelection => sys::ACTION_SELECT_TEXT,
            Action::Copy => sys::ACTION_COPY,
            Action::Cut => sys::ACTION_CUT,
            Action::Paste => sys::ACTION_PASTE,
            // ArkUI has no progress or item actions: a slider steps by scroll actions and a
            // list box's items are elements of their own.
            Action::SetProgress if node.role == Role::Slider => sys::ACTION_SCROLL_FORWARD,
            Action::SetProgress
            | Action::SelectItem { .. }
            | Action::ShowOnScreen
            | Action::ClearSelection => continue,
            // ArkUI action types are a closed set with no custom or dismiss action.
            Action::Custom { .. } | Action::Dismiss => continue,
        });
    }
    if node.role == Role::Slider && out.contains(&sys::ACTION_SCROLL_FORWARD) {
        out.push(sys::ACTION_SCROLL_BACKWARD);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Fills `info` for `node`; `scale` turns logical bounds into device pixels and `origin`
/// is the XComponent's top-left on screen.
///
/// # Safety
/// `info` must be a live element info ArkUI handed to a provider callback.
unsafe fn populate(
    info: *mut sys::ArkUI_AccessibilityElementInfo,
    node: &SemanticNode,
    accessibility_focused: bool,
    scale: f32,
    origin: [f32; 2],
) {
    let c = |text: &str| CString::new(text.replace('\0', "")).unwrap_or_default();
    let contents = c(node
        .text_field
        .as_ref()
        .map_or(node.text.as_str(), |field| field.text.as_str()));
    let name = c(&node.name);
    let [x, y, w, h] = node.bounds;
    let mut rect = sys::ArkUI_AccessibleRect {
        left_top_x: (origin[0] + x * scale) as i32,
        left_top_y: (origin[1] + y * scale) as i32,
        right_bottom_x: (origin[0] + (x + w) * scale) as i32,
        right_bottom_y: (origin[1] + (y + h) * scale) as i32,
    };
    let mut children: Vec<i64> = node.children.iter().map(|&id| i64::from(id)).collect();
    let kinds = actions(node, accessibility_focused);
    let mut actions: Vec<sys::ArkUI_AccessibleAction> = kinds
        .iter()
        .map(|&kind| sys::ArkUI_AccessibleAction {
            kind,
            description: std::ptr::null(),
        })
        .collect();
    // SAFETY: `info` is live for the callback; every pointer outlives its call.
    unsafe {
        sys::OH_ArkUI_AccessibilityElementInfoSetElementId(info, node.id as i32);
        sys::OH_ArkUI_AccessibilityElementInfoSetParentId(
            info,
            node.parent.map_or(ROOT as i32, |p| p as i32),
        );
        sys::OH_ArkUI_AccessibilityElementInfoSetComponentType(
            info,
            component_type(node.role).as_ptr(),
        );
        sys::OH_ArkUI_AccessibilityElementInfoSetContents(info, contents.as_ptr());
        if !node.name.is_empty() {
            sys::OH_ArkUI_AccessibilityElementInfoSetAccessibilityText(info, name.as_ptr());
        }
        if let Some(field) = &node.text_field {
            let hint = c(&field.hint);
            sys::OH_ArkUI_AccessibilityElementInfoSetHintText(info, hint.as_ptr());
            sys::OH_ArkUI_AccessibilityElementInfoSetEditable(info, true);
            sys::OH_ArkUI_AccessibilityElementInfoSetIsPassword(info, field.secure);
            if let Some([start, end]) = field.selection {
                sys::OH_ArkUI_AccessibilityElementInfoSetSelectedTextStart(info, start as i32);
                sys::OH_ArkUI_AccessibilityElementInfoSetSelectedTextEnd(info, end as i32);
            }
        }
        sys::OH_ArkUI_AccessibilityElementInfoSetScreenRect(info, &mut rect);
        sys::OH_ArkUI_AccessibilityElementInfoSetVisible(info, node.visible);
        sys::OH_ArkUI_AccessibilityElementInfoSetEnabled(info, node.enabled);
        sys::OH_ArkUI_AccessibilityElementInfoSetFocusable(info, node.focusable);
        sys::OH_ArkUI_AccessibilityElementInfoSetFocused(info, node.focused);
        sys::OH_ArkUI_AccessibilityElementInfoSetClickable(info, node.clickable);
        sys::OH_ArkUI_AccessibilityElementInfoSetScrollable(
            info,
            node.scrollable || node.scroll.is_some(),
        );
        sys::OH_ArkUI_AccessibilityElementInfoSetAccessibilityFocused(info, accessibility_focused);
        let checked = node.checked.or(node.pressed);
        sys::OH_ArkUI_AccessibilityElementInfoSetCheckable(info, checked.is_some());
        sys::OH_ArkUI_AccessibilityElementInfoSetChecked(info, checked.unwrap_or(false));
        sys::OH_ArkUI_AccessibilityElementInfoSetSelected(info, node.selected.unwrap_or(false));
        if let Some(range) = &node.range {
            let mut range = sys::ArkUI_AccessibleRangeInfo {
                min: range.min,
                max: range.max,
                current: range.value,
            };
            sys::OH_ArkUI_AccessibilityElementInfoSetRangeInfo(info, &mut range);
        }
        if let Some((index, count)) = node.position {
            sys::OH_ArkUI_AccessibilityElementInfoSetCurrentItemIndex(info, index as i32);
            sys::OH_ArkUI_AccessibilityElementInfoSetItemCount(info, count as i32);
        }
        sys::OH_ArkUI_AccessibilityElementInfoSetChildNodeIds(
            info,
            children.len() as i32,
            children.as_mut_ptr(),
        );
        sys::OH_ArkUI_AccessibilityElementInfoSetOperationActions(
            info,
            actions.len() as i32,
            actions.as_mut_ptr(),
        );
    }
    // The test id is the UI test framework's `By.id`; the setter is newer than API 23.
    if !node.test_id.is_empty()
        && let Some(set) = sys::optional(c"OH_ArkUI_AccessibilityElementInfoSetComponentIdentifier")
    {
        let set: unsafe extern "C" fn(
            *mut sys::ArkUI_AccessibilityElementInfo,
            *const c_char,
        ) -> i32 = unsafe { std::mem::transmute(set) };
        let id = c(&node.test_id);
        unsafe { set(info, id.as_ptr()) };
    }
}

/// The page a provider callback's instance id names.
fn token(instance: *const c_char) -> Option<i64> {
    // SAFETY: ArkUI passes back the NUL-terminated id it was registered with.
    unsafe { CStr::from_ptr(instance) }
        .to_str()
        .ok()?
        .parse()
        .ok()
}

unsafe extern "C" fn find_by_id(
    instance: *const c_char,
    id: i64,
    mode: i32,
    _request: i32,
    list: *mut sys::ArkUI_AccessibilityElementInfoList,
) -> i32 {
    let Some(token) = token(instance) else {
        return sys::RESULT_FAILED;
    };
    pages::with(token, |page| {
        let (scale, origin) = page.adapter().placement();
        page.semantics(|projection, focus| {
            let Some(projection) = projection else {
                return sys::RESULT_FAILED;
            };
            let mut wanted: Vec<u32> = if id == ROOT {
                projection.roots()
            } else {
                vec![id as u32]
            };
            if mode & (sys::SEARCH_PREFETCH_CHILDREN | sys::SEARCH_PREFETCH_RECURSIVE_CHILDREN) != 0
            {
                let recursive = mode & sys::SEARCH_PREFETCH_RECURSIVE_CHILDREN != 0;
                let mut i = 0;
                while i < wanted.len() {
                    if let Some(node) = projection.node(wanted[i])
                        && (recursive || i == 0)
                    {
                        wanted.extend(node.children);
                    }
                    i += 1;
                }
            }
            for id in wanted {
                let Some(node) = projection.node(id) else {
                    continue;
                };
                // SAFETY: the list is live for this callback.
                unsafe {
                    let info = sys::OH_ArkUI_AddAndGetAccessibilityElementInfo(list);
                    populate(info, &node, focus == Some(id), scale, origin);
                }
            }
            sys::RESULT_SUCCESSFUL
        })
    })
    .unwrap_or(sys::RESULT_FAILED)
}

unsafe extern "C" fn find_by_text(
    instance: *const c_char,
    _: i64,
    text: *const c_char,
    _request: i32,
    list: *mut sys::ArkUI_AccessibilityElementInfoList,
) -> i32 {
    let (Some(token), false) = (token(instance), text.is_null()) else {
        return sys::RESULT_FAILED;
    };
    // SAFETY: ArkUI's NUL-terminated query for this call.
    let query = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    pages::with(token, |page| {
        let (scale, origin) = page.adapter().placement();
        page.semantics(|projection, focus| {
            let Some(projection) = projection else {
                return sys::RESULT_FAILED;
            };
            let mut pending = projection.roots();
            while let Some(id) = pending.pop() {
                let Some(node) = projection.node(id) else {
                    continue;
                };
                if node.name.contains(&query) || node.text.contains(&query) {
                    // SAFETY: the list is live for this callback.
                    unsafe {
                        let info = sys::OH_ArkUI_AddAndGetAccessibilityElementInfo(list);
                        populate(info, &node, focus == Some(id), scale, origin);
                    }
                }
                pending.extend(node.children.iter().copied());
            }
            sys::RESULT_SUCCESSFUL
        })
    })
    .unwrap_or(sys::RESULT_FAILED)
}

unsafe extern "C" fn find_focused(
    instance: *const c_char,
    _: i64,
    kind: i32,
    _request: i32,
    info: *mut sys::ArkUI_AccessibilityElementInfo,
) -> i32 {
    let Some(token) = token(instance) else {
        return sys::RESULT_FAILED;
    };
    pages::with(token, |page| {
        let (scale, origin) = page.adapter().placement();
        page.semantics(|projection, focus| {
            let projection = projection?;
            let id = if kind == sys::FOCUS_TYPE_INPUT {
                projection.input_focus()
            } else {
                focus
            }?;
            let node = projection.node(id)?;
            // SAFETY: ArkUI's live element info for this callback.
            unsafe { populate(info, &node, focus == Some(id), scale, origin) };
            Some(())
        })
    })
    .flatten()
    .map_or(sys::RESULT_FAILED, |()| sys::RESULT_SUCCESSFUL)
}

unsafe extern "C" fn find_next_focus(
    _: *const c_char,
    _: i64,
    _: i32,
    _: i32,
    _: *mut sys::ArkUI_AccessibilityElementInfo,
) -> i32 {
    sys::RESULT_FAILED
}

unsafe extern "C" fn execute_action(
    instance: *const c_char,
    id: i64,
    action: u32,
    arguments: *mut sys::ArkUI_AccessibilityActionArguments,
    _request: i32,
) -> i32 {
    let Some(token) = token(instance) else {
        return sys::RESULT_FAILED;
    };
    let argument = |key: &CStr| {
        let mut value = std::ptr::null_mut();
        // SAFETY: ArkUI owns the arguments and the returned string for this callback.
        unsafe {
            sys::OH_ArkUI_FindAccessibilityActionArgumentByKey(arguments, key.as_ptr(), &mut value)
        };
        (!value.is_null()).then(|| {
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned()
        })
    };
    let action = match action {
        sys::ACTION_CLICK => AccessibilityAction::Click,
        sys::ACTION_GAIN_ACCESSIBILITY_FOCUS => AccessibilityAction::Focus,
        sys::ACTION_CLEAR_ACCESSIBILITY_FOCUS => AccessibilityAction::ClearFocus,
        sys::ACTION_SCROLL_FORWARD => AccessibilityAction::ScrollForward,
        sys::ACTION_SCROLL_BACKWARD => AccessibilityAction::ScrollBackward,
        sys::ACTION_COPY => AccessibilityAction::Copy,
        sys::ACTION_CUT => AccessibilityAction::Cut,
        sys::ACTION_SET_TEXT => {
            AccessibilityAction::SetText(argument(c"setText").unwrap_or_default())
        }
        sys::ACTION_SELECT_TEXT => {
            let bound = |key| argument(key).and_then(|v| v.parse().ok()).unwrap_or(-1);
            AccessibilityAction::SetSelection(bound(c"selectTextBegin"), bound(c"selectTextEnd"))
        }
        sys::ACTION_SET_CURSOR_POSITION => {
            let at = argument(c"offset")
                .and_then(|v| v.parse().ok())
                .unwrap_or(-1);
            AccessibilityAction::SetSelection(at, at)
        }
        // Paste needs the clipboard, which ArkTS reads asynchronously.
        sys::ACTION_PASTE => return pages::paste(token, id as u32),
        _ => return sys::RESULT_FAILED,
    };
    pages::with(token, |page| page.accessibility(id as u32, action)).map_or(
        sys::RESULT_FAILED,
        |done| {
            if done {
                sys::RESULT_SUCCESSFUL
            } else {
                sys::RESULT_FAILED
            }
        },
    )
}

unsafe extern "C" fn clear_focused(instance: *const c_char) -> i32 {
    let Some(token) = token(instance) else {
        return sys::RESULT_FAILED;
    };
    pages::with(token, |page| {
        let focus = page.semantics(|_, focus| focus);
        if let Some(id) = focus {
            page.accessibility(id, AccessibilityAction::ClearFocus);
        }
    });
    sys::RESULT_SUCCESSFUL
}

unsafe extern "C" fn cursor_position(
    instance: *const c_char,
    id: i64,
    _: i32,
    index: *mut i32,
) -> i32 {
    let Some(token) = token(instance) else {
        return sys::RESULT_FAILED;
    };
    let caret = pages::with(token, |page| {
        page.semantics(|projection, _| {
            let field = projection?.node(id as u32)?.text_field?;
            field.selection.map(|[_, end]| end as i32)
        })
    })
    .flatten();
    match caret {
        Some(caret) => {
            // SAFETY: ArkUI's out parameter for this call.
            unsafe { *index = caret };
            sys::RESULT_SUCCESSFUL
        }
        None => sys::RESULT_FAILED,
    }
}

/// Registers the provider of the page `token`'s XComponent node.
pub(crate) fn attach(
    token: i64,
    provider: *mut sys::ArkUI_AccessibilityProvider,
) -> Result<(), String> {
    static mut CALLBACKS: sys::ArkUI_AccessibilityProviderCallbacksWithInstance =
        sys::ArkUI_AccessibilityProviderCallbacksWithInstance {
            find_by_id,
            find_by_text,
            find_focused,
            find_next_focus,
            execute_action,
            clear_focused,
            cursor_position,
        };
    let instance = CString::new(token.to_string()).expect("digits");
    // SAFETY: ArkUI copies the id and keeps the static callback table.
    let code = unsafe {
        sys::OH_ArkUI_AccessibilityProviderRegisterCallbackWithInstance(
            instance.as_ptr(),
            provider,
            std::ptr::addr_of_mut!(CALLBACKS),
        )
    };
    (code == sys::RESULT_SUCCESSFUL)
        .then_some(())
        .ok_or_else(|| format!("UI_ACCESSIBILITY: provider registration failed ({code})"))
}

/// Publishes the core's accessibility events to the screen reader.
pub(crate) fn announce(
    provider: *mut sys::ArkUI_AccessibilityProvider,
    events: &[AccessibilityEffect],
) {
    for event in events {
        let (kind, id, text) = match event {
            AccessibilityEffect::Focused { id } => (sys::EVENT_ACCESSIBILITY_FOCUSED, *id, None),
            AccessibilityEffect::FocusCleared { id } => {
                (sys::EVENT_ACCESSIBILITY_FOCUS_CLEARED, *id, None)
            }
            AccessibilityEffect::InputFocused { id } => (sys::EVENT_FOCUS_NODE_UPDATE, *id, None),
            AccessibilityEffect::Change(Event::ValueChanged { id }) => {
                (sys::EVENT_SELECTED, *id, None)
            }
            AccessibilityEffect::Change(Event::TextChanged { id, .. }) => {
                (sys::EVENT_TEXT_UPDATE, *id, None)
            }
            AccessibilityEffect::Change(Event::ContentChanged) => {
                (sys::EVENT_PAGE_CONTENT_UPDATE, 0, None)
            }
            AccessibilityEffect::Change(Event::Announce { id, text, .. }) => (
                sys::EVENT_ANNOUNCE_FOR_ACCESSIBILITY,
                *id,
                Some(text.as_str()),
            ),
            _ => continue,
        };
        // SAFETY: the event and its element info live until the send copies them.
        unsafe {
            let info = sys::OH_ArkUI_CreateAccessibilityEventInfo();
            let element = sys::OH_ArkUI_CreateAccessibilityElementInfo();
            sys::OH_ArkUI_AccessibilityElementInfoSetElementId(element, id as i32);
            sys::OH_ArkUI_AccessibilityEventSetEventType(info, kind);
            sys::OH_ArkUI_AccessibilityEventSetElementInfo(info, element);
            let text = text.map(|t| CString::new(t.replace('\0', "")).unwrap_or_default());
            if let Some(text) = &text {
                sys::OH_ArkUI_AccessibilityEventSetTextAnnouncedForAccessibility(
                    info,
                    text.as_ptr(),
                );
            }
            sys::OH_ArkUI_SendAccessibilityAsyncEvent(provider, info, None);
            sys::OH_ArkUI_DestoryAccessibilityElementInfo(element);
            sys::OH_ArkUI_DestoryAccessibilityEventInfo(info);
        }
    }
}
