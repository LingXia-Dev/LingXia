//! UI Automation for a GPU page: the page window is a fragment root whose fragments are the
//! View's semantic nodes. Providers hold only the window and a node id; every call reads the
//! page on its own thread.
#![allow(non_upper_case_globals)]
use crate::Page;
use lingxia_view::interaction::view::{AccessibilityAction, AccessibilityEffect};
use lingxia_view::semantics::projection::{Event, Role, SemanticNode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{SafeArrayCreateVector, SafeArrayPutElement};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, SendMessageW, WM_APP};
use windows::core::{BOOL, BSTR, Error, HRESULT, IUnknown, Interface, PCWSTR, Result, implement};

/// Runs a provider call on the page thread: `lparam` points at a `&mut dyn FnMut()`.
pub(crate) const CALL: u32 = WM_APP + 0x4c58;

thread_local! {
    /// Each page window's root provider, so UIA sees one identity per page.
    static ROOTS: RefCell<HashMap<isize, IRawElementProviderSimple>> = RefCell::new(HashMap::new());
}

/// `WM_GETOBJECT`: hand UIA the page's root.
pub(crate) fn get_object(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if lparam.0 as i32 != UiaRootObjectId {
        return None;
    }
    let root = ROOTS.with(|roots| {
        roots
            .borrow_mut()
            .entry(hwnd.0 as isize)
            .or_insert_with(|| {
                Element {
                    hwnd: hwnd.0 as isize,
                    id: None,
                }
                .into()
            })
            .clone()
    });
    // SAFETY: answers the message being handled with a live provider.
    Some(unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &root) })
}

/// The page window is going: UIA lets go of its providers.
pub(crate) fn window_destroyed(hwnd: HWND) {
    if let Some(root) = ROOTS.with(|roots| roots.borrow_mut().remove(&(hwnd.0 as isize))) {
        // SAFETY: plain UIA calls on this thread's window.
        unsafe {
            let _ = UiaReturnRawElementProvider(hwnd, WPARAM(0), LPARAM(0), None);
            let _ = UiaDisconnectProvider(&root);
        }
    }
}

/// `CALL`: run the closure the sender lent.
pub(crate) fn call(lparam: LPARAM) -> LRESULT {
    // SAFETY: the sender blocks in SendMessageW until this returns, so the pointer is live.
    let work = unsafe { &mut *(lparam.0 as *mut &mut dyn FnMut()) };
    work();
    LRESULT(0)
}

/// `work` on the page thread, with the page; `None` when the page is gone.
fn on_page<T>(hwnd: isize, work: impl FnOnce(&Rc<Page>) -> T) -> Option<T> {
    let mut work = Some(work);
    let mut out = None;
    let mut run = || {
        if let (Some(work), Some(page)) = (work.take(), crate::view::page_for(hwnd)) {
            out = Some(work(&page));
        }
    };
    let window = HWND(hwnd as *mut _);
    // SAFETY: reads a window's thread; a sent message runs on it while this thread waits.
    unsafe {
        if GetWindowThreadProcessId(window, None) == GetCurrentThreadId() {
            run();
        } else {
            let mut run: &mut dyn FnMut() = &mut run;
            SendMessageW(
                window,
                CALL,
                None,
                Some(LPARAM(&mut run as *mut &mut dyn FnMut() as isize)),
            );
        }
    }
    out
}

/// The semantic tree as presented: nodes by id, each node's container, the roots.
#[derive(Default)]
struct Tree {
    nodes: HashMap<u32, SemanticNode>,
    parents: HashMap<u32, u32>,
    roots: Vec<u32>,
}

impl Tree {
    fn of(page: &Page) -> Self {
        page.host().semantics(|projection, _| {
            let mut tree = Tree::default();
            let Some(projection) = projection else {
                return tree;
            };
            let mut pending = projection.roots();
            pending.reverse();
            while let Some(id) = pending.pop() {
                let Some(node) = projection.node(id) else {
                    continue;
                };
                for child in node.children.iter().rev() {
                    tree.parents.insert(*child, id);
                    pending.push(*child);
                }
                tree.nodes.insert(id, node);
            }
            tree.roots = projection
                .roots()
                .into_iter()
                .filter(|id| tree.nodes.contains_key(id))
                .collect();
            tree
        })
    }

    fn children(&self, id: Option<u32>) -> Vec<u32> {
        match id {
            None => self.roots.clone(),
            Some(id) => self.nodes.get(&id).map_or_else(Vec::new, |node| {
                node.children
                    .iter()
                    .copied()
                    .filter(|child| self.nodes.contains_key(child))
                    .collect()
            }),
        }
    }
}

fn unavailable() -> Error {
    Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32))
}

/// A null out-pointer with success, as UIA expects for "none".
fn none<T>() -> Result<T> {
    Err(Error::empty())
}

#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot,
    IInvokeProvider,
    IToggleProvider,
    IValueProvider,
    IRangeValueProvider,
    IScrollItemProvider,
    IExpandCollapseProvider,
    ISelectionItemProvider
)]
#[derive(Clone, Copy)]
struct Element {
    hwnd: isize,
    /// `None` is the page itself.
    id: Option<u32>,
}

impl Element {
    fn at(&self, id: Option<u32>) -> Element {
        Element {
            hwnd: self.hwnd,
            id,
        }
    }

    fn fragment(&self, id: Option<u32>) -> Result<IRawElementProviderFragment> {
        if id.is_none() {
            return self.root().cast();
        }
        Ok(self.at(id).into())
    }

    fn root(&self) -> IRawElementProviderSimple {
        let hwnd = self.hwnd;
        ROOTS
            .with(|roots| roots.borrow().get(&hwnd).cloned())
            .unwrap_or_else(|| self.at(None).into())
    }

    fn node(&self) -> Result<SemanticNode> {
        let id = self.id.ok_or_else(unavailable)?;
        on_page(self.hwnd, |page| {
            page.host()
                .semantics(|projection, _| projection.and_then(|p| p.node(id)))
        })
        .flatten()
        .ok_or_else(unavailable)
    }

    fn act(&self, action: AccessibilityAction) -> Result<()> {
        let id = self.id.ok_or_else(unavailable)?;
        let handled = on_page(self.hwnd, |page| page.host().accessibility(id, action));
        match handled {
            Some(true) => Ok(()),
            Some(false) => Err(Error::from_hresult(HRESULT(UIA_E_ELEMENTNOTENABLED as i32))),
            None => Err(unavailable()),
        }
    }

    /// Screen pixels of a logical rectangle of the page.
    fn screen(&self, bounds: [f32; 4]) -> UiaRect {
        on_page(self.hwnd, |page| {
            let scale = f64::from(page.shown_scale());
            let mut origin = POINT::default();
            // SAFETY: maps this page window's client origin.
            let _ = unsafe { ClientToScreen(HWND(self.hwnd as *mut _), &mut origin) };
            UiaRect {
                left: f64::from(origin.x) + f64::from(bounds[0]) * scale,
                top: f64::from(origin.y) + f64::from(bounds[1]) * scale,
                width: f64::from(bounds[2]) * scale,
                height: f64::from(bounds[3]) * scale,
            }
        })
        .unwrap_or_default()
    }

    fn toggles(node: &SemanticNode) -> bool {
        matches!(
            node.role,
            Role::CheckBox | Role::Switch | Role::MenuItemCheckBox
        ) || (node.pressed.is_some() && node.role == Role::Button)
    }

    fn selects(node: &SemanticNode) -> bool {
        matches!(
            node.role,
            Role::RadioButton | Role::Option | Role::GridCell | Role::Tab | Role::MenuItemRadio
        ) && (node.checked.is_some() || node.selected.is_some())
    }
}

fn control_type(role: Role) -> UIA_CONTROLTYPE_ID {
    match role {
        Role::Button | Role::Switch => UIA_ButtonControlTypeId,
        Role::CheckBox => UIA_CheckBoxControlTypeId,
        Role::RadioGroup => UIA_GroupControlTypeId,
        Role::RadioButton => UIA_RadioButtonControlTypeId,
        Role::ComboBox => UIA_ComboBoxControlTypeId,
        Role::TextField => UIA_EditControlTypeId,
        Role::ScrollView | Role::HorizontalScrollView => UIA_PaneControlTypeId,
        Role::Image => UIA_ImageControlTypeId,
        Role::Slider => UIA_SliderControlTypeId,
        Role::Text | Role::Heading => UIA_TextControlTypeId,
        Role::Link => UIA_HyperlinkControlTypeId,
        Role::List | Role::ListBox => UIA_ListControlTypeId,
        Role::ListItem | Role::Option => UIA_ListItemControlTypeId,
        Role::Dialog | Role::AlertDialog => UIA_WindowControlTypeId,
        Role::Alert | Role::Group => UIA_GroupControlTypeId,
        Role::Tooltip => UIA_ToolTipControlTypeId,
        Role::ProgressBar => UIA_ProgressBarControlTypeId,
        Role::TabList => UIA_TabControlTypeId,
        Role::Tab => UIA_TabItemControlTypeId,
        Role::Status => UIA_StatusBarControlTypeId,
        Role::Grid => UIA_DataGridControlTypeId,
        Role::GridCell => UIA_DataItemControlTypeId,
        Role::Menu => UIA_MenuControlTypeId,
        Role::MenuItem | Role::MenuItemCheckBox | Role::MenuItemRadio => UIA_MenuItemControlTypeId,
        Role::Toolbar => UIA_ToolBarControlTypeId,
        Role::Tree => UIA_TreeControlTypeId,
        Role::TreeItem => UIA_TreeItemControlTypeId,
        Role::Row => UIA_DataItemControlTypeId,
        Role::Video | Role::TabPanel | Role::WebView => UIA_PaneControlTypeId,
    }
}

impl IRawElementProviderSimple_Impl for Element_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID) -> Result<IUnknown> {
        let Ok(node) = self.node() else {
            return none();
        };
        let supported = match pattern {
            UIA_InvokePatternId => {
                node.clickable
                    && !Element::toggles(&node)
                    && !Element::selects(&node)
                    && node.expanded.is_none()
            }
            UIA_TogglePatternId => Element::toggles(&node),
            UIA_SelectionItemPatternId => Element::selects(&node),
            UIA_ValuePatternId => node.text_field.is_some() || node.value.is_some(),
            UIA_RangeValuePatternId => node.range.is_some(),
            UIA_ExpandCollapsePatternId => node.expanded.is_some(),
            UIA_ScrollItemPatternId => true,
            _ => false,
        };
        if !supported {
            return none();
        }
        let element: IRawElementProviderSimple = self.this.into();
        element.cast()
    }

    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID) -> Result<VARIANT> {
        if self.id.is_none() {
            return Ok(match property {
                UIA_ControlTypePropertyId => VARIANT::from(UIA_PaneControlTypeId.0),
                UIA_FrameworkIdPropertyId => VARIANT::from(BSTR::from("LingXia")),
                UIA_IsKeyboardFocusablePropertyId => VARIANT::from(true),
                _ => VARIANT::default(),
            });
        }
        let Ok(node) = self.node() else {
            return Ok(VARIANT::default());
        };
        let text = |value: &str| VARIANT::from(BSTR::from(value));
        Ok(match property {
            UIA_ControlTypePropertyId => VARIANT::from(control_type(node.role).0),
            UIA_LocalizedControlTypePropertyId if node.role == Role::Switch => text("switch"),
            UIA_NamePropertyId => text(if node.name.is_empty() {
                &node.text
            } else {
                &node.name
            }),
            UIA_AutomationIdPropertyId if !node.test_id.is_empty() => text(&node.test_id),
            UIA_FrameworkIdPropertyId => text("LingXia"),
            UIA_HelpTextPropertyId => node
                .text_field
                .as_ref()
                .map(|field| field.hint.as_str())
                .filter(|hint| !hint.is_empty())
                .or(node.description.as_deref())
                .map_or_else(VARIANT::default, text),
            UIA_FullDescriptionPropertyId => node
                .description
                .as_deref()
                .map_or_else(VARIANT::default, text),
            UIA_IsEnabledPropertyId => VARIANT::from(node.enabled),
            UIA_IsKeyboardFocusablePropertyId => VARIANT::from(node.focusable),
            UIA_HasKeyboardFocusPropertyId => VARIANT::from(node.focused),
            UIA_IsOffscreenPropertyId => VARIANT::from(!node.visible),
            UIA_IsPasswordPropertyId => {
                VARIANT::from(node.text_field.as_ref().is_some_and(|field| field.secure))
            }
            UIA_IsControlElementPropertyId | UIA_IsContentElementPropertyId => VARIANT::from(true),
            UIA_HeadingLevelPropertyId => node.heading_level.map_or_else(VARIANT::default, |l| {
                VARIANT::from(HeadingLevel1.0 + i32::from(l.clamp(1, 9)) - 1)
            }),
            UIA_LiveSettingPropertyId => match node.live_region.as_deref() {
                Some("assertive") => VARIANT::from(Assertive.0),
                Some(_) => VARIANT::from(Polite.0),
                None => VARIANT::default(),
            },
            UIA_PositionInSetPropertyId => node
                .position
                .map_or_else(VARIANT::default, |(row, _)| VARIANT::from(row as i32 + 1)),
            UIA_SizeOfSetPropertyId => node
                .position
                .map_or_else(VARIANT::default, |(_, count)| VARIANT::from(count as i32)),
            UIA_IsDialogPropertyId => {
                VARIANT::from(matches!(node.role, Role::Dialog | Role::AlertDialog))
            }
            _ => VARIANT::default(),
        })
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        if self.id.is_some() {
            return none();
        }
        // SAFETY: the page's own window.
        unsafe { UiaHostProviderFromHwnd(HWND(self.hwnd as *mut _)) }
    }
}

impl IRawElementProviderFragment_Impl for Element_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let target = on_page(self.hwnd, |page| {
            let tree = Tree::of(page);
            let parent = |id: u32| tree.parents.get(&id).copied();
            match (direction, self.id) {
                (NavigateDirection_Parent, Some(id)) => Some(parent(id)),
                (NavigateDirection_FirstChild, id) => tree.children(id).first().map(|c| Some(*c)),
                (NavigateDirection_LastChild, id) => tree.children(id).last().map(|c| Some(*c)),
                (NavigateDirection_NextSibling | NavigateDirection_PreviousSibling, Some(id)) => {
                    let siblings = tree.children(parent(id));
                    let at = siblings.iter().position(|s| *s == id)?;
                    let next = if direction == NavigateDirection_NextSibling {
                        siblings.get(at + 1)
                    } else {
                        at.checked_sub(1).and_then(|i| siblings.get(i))
                    };
                    next.map(|s| Some(*s))
                }
                _ => None,
            }
        })
        .flatten();
        match target {
            Some(id) => self.fragment(id),
            None => none(),
        }
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        let Some(id) = self.id else {
            return Ok(std::ptr::null_mut());
        };
        // SAFETY: a two-element I4 vector UIA takes ownership of.
        unsafe {
            let array = SafeArrayCreateVector(VT_I4, 0, 2);
            for (index, value) in [UiaAppendRuntimeId as i32, id as i32].iter().enumerate() {
                SafeArrayPutElement(array, &(index as i32), value as *const i32 as *const _)?;
            }
            Ok(array)
        }
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        if self.id.is_none() {
            return Ok(UiaRect::default());
        }
        let node = self.node()?;
        if !node.visible {
            return Ok(UiaRect::default());
        }
        Ok(self.screen(node.bounds))
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }

    fn SetFocus(&self) -> Result<()> {
        if self.id.is_none() {
            on_page(self.hwnd, |page| page.focus());
            return Ok(());
        }
        self.act(AccessibilityAction::Focus)
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.root().cast()
    }
}

impl IRawElementProviderFragmentRoot_Impl for Element_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let hit = on_page(self.hwnd, |page| {
            let mut point = POINT {
                x: x.round() as i32,
                y: y.round() as i32,
            };
            // SAFETY: maps a screen point into this page window.
            let _ = unsafe { ScreenToClient(HWND(self.hwnd as *mut _), &mut point) };
            let scale = page.shown_scale();
            let at = [point.x as f32 / scale, point.y as f32 / scale];
            page.host()
                .semantics(|projection, _| projection.and_then(|p| p.hit(at, 0.0)))
        })
        .flatten();
        self.fragment(hit)
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        let focused = on_page(self.hwnd, |page| {
            Tree::of(page)
                .nodes
                .values()
                .find(|node| node.focused)
                .map(|node| node.id)
        })
        .flatten();
        match focused {
            Some(id) => self.fragment(Some(id)),
            None => none(),
        }
    }
}

impl IInvokeProvider_Impl for Element_Impl {
    fn Invoke(&self) -> Result<()> {
        self.act(AccessibilityAction::Click)
    }
}

impl IToggleProvider_Impl for Element_Impl {
    fn Toggle(&self) -> Result<()> {
        self.act(AccessibilityAction::Click)
    }

    fn ToggleState(&self) -> Result<ToggleState> {
        let node = self.node()?;
        Ok(match node.checked.or(node.pressed) {
            Some(true) => ToggleState_On,
            _ => ToggleState_Off,
        })
    }
}

impl ISelectionItemProvider_Impl for Element_Impl {
    fn Select(&self) -> Result<()> {
        if self.IsSelected()?.as_bool() {
            return Ok(());
        }
        self.act(AccessibilityAction::Click)
    }

    fn AddToSelection(&self) -> Result<()> {
        self.Select()
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(Error::from_hresult(HRESULT(UIA_E_INVALIDOPERATION as i32)))
    }

    fn IsSelected(&self) -> Result<BOOL> {
        let node = self.node()?;
        Ok(node.checked.or(node.selected).unwrap_or(false).into())
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        let node = self.node()?;
        let container = on_page(self.hwnd, |page| {
            Tree::of(page).parents.get(&node.id).copied()
        })
        .flatten();
        match node.parent.or(container) {
            Some(id) => Ok(self.at(Some(id)).into()),
            None => none(),
        }
    }
}

impl IValueProvider_Impl for Element_Impl {
    fn SetValue(&self, value: &PCWSTR) -> Result<()> {
        // SAFETY: UIA passes a NUL-terminated string valid for this call.
        let value = unsafe { value.to_string() }.unwrap_or_default();
        self.act(AccessibilityAction::SetText(value))
    }

    fn Value(&self) -> Result<BSTR> {
        let node = self.node()?;
        Ok(BSTR::from(match (&node.text_field, &node.value) {
            (Some(field), _) => field.text.clone(),
            (None, Some(value)) => value.clone(),
            (None, None) => String::new(),
        }))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        let node = self.node()?;
        Ok((!node.enabled || node.text_field.is_none()).into())
    }
}

impl IRangeValueProvider_Impl for Element_Impl {
    fn SetValue(&self, value: f64) -> Result<()> {
        self.act(AccessibilityAction::SetProgress(value))
    }

    fn Value(&self) -> Result<f64> {
        Ok(self.node()?.range.map_or(0.0, |range| range.value))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok((!self.node()?.enabled).into())
    }

    fn Maximum(&self) -> Result<f64> {
        Ok(self.node()?.range.map_or(0.0, |range| range.max))
    }

    fn Minimum(&self) -> Result<f64> {
        Ok(self.node()?.range.map_or(0.0, |range| range.min))
    }

    fn LargeChange(&self) -> Result<f64> {
        Ok(self
            .node()?
            .range
            .map_or(0.0, |range| (range.max - range.min) / 10.0))
    }

    fn SmallChange(&self) -> Result<f64> {
        Ok(self
            .node()?
            .range
            .map_or(0.0, |range| (range.max - range.min) / 100.0))
    }
}

impl IScrollItemProvider_Impl for Element_Impl {
    fn ScrollIntoView(&self) -> Result<()> {
        self.act(AccessibilityAction::ShowOnScreen)
    }
}

impl IExpandCollapseProvider_Impl for Element_Impl {
    fn Expand(&self) -> Result<()> {
        if self.node()?.expanded == Some(true) {
            return Ok(());
        }
        self.act(AccessibilityAction::Click)
    }

    fn Collapse(&self) -> Result<()> {
        if self.node()?.expanded == Some(false) {
            return Ok(());
        }
        self.act(AccessibilityAction::Click)
    }

    fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
        Ok(match self.node()?.expanded {
            Some(true) => ExpandCollapseState_Expanded,
            _ => ExpandCollapseState_Collapsed,
        })
    }
}

/// Tell listening clients what changed on the page in `hwnd`. On the page thread.
pub(crate) fn announce(hwnd: isize, events: &[AccessibilityEffect]) {
    // SAFETY: a plain query.
    if hwnd == 0 || !unsafe { UiaClientsAreListening() }.as_bool() {
        return;
    }
    let element = |id: u32| -> IRawElementProviderSimple { Element { hwnd, id: Some(id) }.into() };
    for event in events {
        // SAFETY: raising events for providers of this page.
        unsafe {
            let _ = match event {
                AccessibilityEffect::Focused { id } | AccessibilityEffect::InputFocused { id } => {
                    UiaRaiseAutomationEvent(&element(*id), UIA_AutomationFocusChangedEventId)
                }
                AccessibilityEffect::Change(Event::TextChanged {
                    id, before, after, ..
                }) => UiaRaiseAutomationPropertyChangedEvent(
                    &element(*id),
                    UIA_ValueValuePropertyId,
                    &VARIANT::from(BSTR::from(before.as_str())),
                    &VARIANT::from(BSTR::from(after.as_str())),
                ),
                AccessibilityEffect::Change(Event::SelectionChanged { id, .. }) => {
                    UiaRaiseAutomationEvent(&element(*id), UIA_Text_TextSelectionChangedEventId)
                }
                AccessibilityEffect::Change(Event::ValueChanged { id }) => {
                    UiaRaiseAutomationPropertyChangedEvent(
                        &element(*id),
                        UIA_RangeValueValuePropertyId,
                        &VARIANT::default(),
                        &VARIANT::default(),
                    )
                }
                AccessibilityEffect::Change(Event::Announce {
                    id,
                    text,
                    assertive,
                }) => UiaRaiseNotificationEvent(
                    &element(*id),
                    NotificationKind_Other,
                    if *assertive {
                        NotificationProcessing_ImportantMostRecent
                    } else {
                        NotificationProcessing_MostRecent
                    },
                    &BSTR::from(text.as_str()),
                    &BSTR::from("lingxia-live-region"),
                ),
                AccessibilityEffect::Change(Event::ContentChanged) => {
                    let Some(root) = ROOTS.with(|roots| roots.borrow().get(&hwnd).cloned()) else {
                        continue;
                    };
                    UiaRaiseStructureChangedEvent(
                        &root,
                        StructureChangeType_ChildrenInvalidated,
                        std::ptr::null_mut(),
                        0,
                    )
                }
                _ => continue,
            };
        }
    }
}
