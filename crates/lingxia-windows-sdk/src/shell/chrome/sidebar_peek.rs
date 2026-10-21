//! Sidebar peek: the expanded sidebar drawn as a transient overlay above the
//! collapsed rail. The window layout stays collapsed, so nothing here may feed
//! back into workspace geometry or persisted sidebar state.

use super::*;

fn sidebar_peek_shadow() -> i32 {
    px(12)
}

/// Peak shadow opacity on the edge facing the workspace.
const SHADOW_MAX_ALPHA: f32 = 40.0;

/// The rail's expanded twin, positioned over the owner's client area.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SidebarPeekLayout {
    /// Expanded copy of the window's sidebar; scroll state is the peek's own.
    layout: WindowsShellWindowLayout,
    position: WindowsShellTabBarPosition,
    /// Expanded column in owner client coordinates.
    pub(crate) body: RECT,
    /// Peek window bounds in owner client coordinates: body plus shadow.
    pub(crate) frame: RECT,
    /// Compact desktop: pinning is unavailable, so the expand control closes
    /// the peek instead.
    pub(crate) compact: bool,
    /// The rail column inside the body, in owner client coordinates. The peek
    /// keeps the rail's control row, so the expand control never moves.
    rail: RECT,
    /// The owner's client area, which bounds the expand control's tooltip.
    client: RECT,
    /// Drawn and hit-tested by the peek itself: on a right rail the pinned
    /// positions would run under the expand control.
    header_actions: Vec<WindowsShellHeaderActionLayout>,
}

/// Expanded peek over a collapsed side rail; `None` when the sidebar is not a
/// rail (nothing to peek at).
pub(crate) fn sidebar_peek_layout(
    client: RECT,
    layout: &WindowsWindowLayout,
) -> Option<SidebarPeekLayout> {
    peek_layout_for(client, shell_layout(layout)?)
}

fn peek_layout_for(client: RECT, layout: &WindowsShellWindowLayout) -> Option<SidebarPeekLayout> {
    let tabbar = layout.tab_bar.as_ref()?;
    let left_side = match tabbar.position {
        WindowsShellTabBarPosition::Left => true,
        WindowsShellTabBarPosition::Right => false,
        WindowsShellTabBarPosition::Bottom => return None,
    };
    if !tabbar.visible || !(tabbar.collapsed || tabbar.icon_rail) {
        return None;
    }
    let rail = compute_chrome_rects(client, layout).tab_bar?;
    let width = tabbar
        .dimension
        .max(shell_sidebar_width())
        .min(rect_width(&client));
    let shadow = sidebar_peek_shadow();
    let (body, frame) = if left_side {
        let body = RECT {
            right: rail.left + width,
            ..rail
        };
        let frame = RECT {
            right: (body.right + shadow).min(client.right).max(body.right),
            ..body
        };
        (body, frame)
    } else {
        let body = RECT {
            left: rail.right - width,
            ..rail
        };
        let frame = RECT {
            left: (body.left - shadow).max(client.left).min(body.left),
            ..body
        };
        (body, frame)
    };

    let mut expanded = tabbar.clone();
    expanded.collapsed = false;
    expanded.icon_rail = false;
    expanded.footer_action_height = panel_footer_action_height(width, &layout.footer_actions);
    expanded.main_scroll_offset = 0;
    expanded.footer_action_scroll_row = 0;
    let header_actions = std::mem::take(&mut expanded.header_actions);
    Some(SidebarPeekLayout {
        layout: WindowsShellWindowLayout {
            tab_bar: Some(expanded),
            footer_actions: layout.footer_actions.clone(),
            ..Default::default()
        },
        position: tabbar.position,
        body: normalize_rect(body),
        frame: normalize_rect(frame),
        compact: tabbar.rail_expand_disabled,
        rail,
        client,
        header_actions,
    })
}

impl SidebarPeekLayout {
    fn tabbar(&self) -> &WindowsShellTabBarLayout {
        self.layout
            .tab_bar
            .as_ref()
            .expect("a peek always carries its expanded sidebar")
    }

    fn tabbar_mut(&mut self) -> &mut WindowsShellTabBarLayout {
        self.layout
            .tab_bar
            .as_mut()
            .expect("a peek always carries its expanded sidebar")
    }

    pub(crate) fn size(&self) -> (i32, i32) {
        (rect_width(&self.frame), rect_height(&self.frame))
    }

    /// The body in peek-window coordinates.
    fn local_body(&self) -> RECT {
        offset_rect(self.body, -self.frame.left, -self.frame.top)
    }

    fn local_rects(&self) -> ChromeRects {
        let body = self.local_body();
        ChromeRects {
            workspace: RECT::default(),
            content: RECT::default(),
            panel: RECT::default(),
            top_bar: RECT::default(),
            navigation_bar: None,
            tab_bar: Some(body),
            phone_bar: None,
        }
    }

    /// The rail's expand control, exactly where the rail draws it.
    pub(crate) fn expand_rect(&self) -> RECT {
        sidebar_rail_expand_rect(offset_rect(self.rail, -self.frame.left, -self.frame.top))
    }

    /// Header actions where the pinned sidebar draws them, moved toward the
    /// leading edge only as far as needed to clear the expand control.
    fn header_action_rects(&self) -> Vec<(usize, RECT)> {
        let body = self.local_body();
        let mut pinned = self.tabbar().clone();
        pinned.header_actions = self.header_actions.clone();
        let rects = sidebar_header_action_rects(body, &pinned);
        let expand = self.expand_rect();
        let limit = expand.left - sidebar_header_action_gap();
        let overlap = rects
            .iter()
            .filter(|(_, rect)| rect.right > limit && rect.left < expand.right)
            .map(|(_, rect)| rect.right - limit)
            .max()
            .unwrap_or(0);
        let leading_limit = body.left + header_leading_reserve();
        rects
            .into_iter()
            .filter_map(|(id, rect)| {
                let index = self
                    .header_actions
                    .iter()
                    .position(|action| action.id == id)?;
                let rect = offset_rect(rect, -overlap, 0);
                (rect.left >= leading_limit).then_some((index, rect))
            })
            .collect()
    }

    /// Hit in peek-window coordinates. `None` outside the body (the shadow).
    pub(crate) fn hit(&self, point: (i32, i32)) -> Option<WindowsChromeHit> {
        let body = self.local_body();
        if !rect_contains(&body, point) {
            return None;
        }
        if rect_contains(&self.expand_rect(), point) {
            // Compact cannot pin; the same control closes the peek.
            let command = if self.compact {
                command_id::SIDEBAR_PEEK
            } else {
                command_id::SIDEBAR_TOGGLE
            };
            return Some(chrome_command(command, serde_json::json!({})));
        }
        for (index, rect) in self.header_action_rects() {
            if !rect_contains(&rect, point) {
                continue;
            }
            let action = &self.header_actions[index];
            if action.disabled {
                return Some(WindowsChromeHit::Chrome);
            }
            let command = match action.source {
                WindowsShellSidebarActionSource::StaticSettings(_) => {
                    command_id::STATIC_SETTINGS_CLICK
                }
                WindowsShellSidebarActionSource::Runtime => command_id::SIDEBAR_ACTION,
            };
            return Some(chrome_command(
                command,
                serde_json::json!({
                    "generation": action.generation,
                    "action_id": action.id,
                    "panel_id": action.id,
                }),
            ));
        }
        // The caption row is the owner's drag region; inside the peek it is
        // inert.
        Some(
            match sidebar_hit_test(body, &self.local_rects(), &self.layout, point) {
                Some(WindowsChromeHit::Caption) | None => WindowsChromeHit::Chrome,
                Some(hit) => hit,
            },
        )
    }

    pub(crate) fn hover_rect(&self, point: (i32, i32)) -> Option<RECT> {
        let expand = self.expand_rect();
        if rect_contains(&expand, point) {
            return Some(expand);
        }
        if let Some((_, rect)) = self
            .header_action_rects()
            .into_iter()
            .find(|(index, rect)| {
                !self.header_actions[*index].disabled && rect_contains(rect, point)
            })
        {
            return Some(rect);
        }
        sidebar_hover_rect(self.local_body(), &self.local_rects(), &self.layout, point)
    }

    /// Scrolls the peek's own navigation or footer rows. True when it moved.
    pub(crate) fn wheel(&mut self, point: (i32, i32), delta: i16) -> bool {
        match sidebar_wheel(
            self.local_body(),
            &self.local_rects(),
            &self.layout,
            point,
            delta,
        ) {
            Some(SidebarWheel::Scroll(offset)) => self.tabbar_mut().main_scroll_offset = offset,
            Some(SidebarWheel::FooterRow(row)) => self.tabbar_mut().footer_action_scroll_row = row,
            None => return false,
        }
        true
    }

    /// Same window placement and size class; only content may differ.
    pub(crate) fn same_placement(&self, other: &Self) -> bool {
        self.body == other.body
            && self.frame == other.frame
            && self.rail == other.rail
            && self.client == other.client
            && self.compact == other.compact
    }

    /// Adopt fresh sidebar content while keeping where the peek is scrolled.
    pub(crate) fn refresh_from(&mut self, mut next: Self) {
        let (offset, row) = {
            let tabbar = self.tabbar();
            (tabbar.main_scroll_offset, tabbar.footer_action_scroll_row)
        };
        let tabbar = next.tabbar_mut();
        tabbar.main_scroll_offset = offset;
        tabbar.footer_action_scroll_row = row;
        *self = next;
    }

    /// Hover label in peek-window coordinates for the expand control (absent
    /// in compact, which has nothing to pin) or a header action.
    pub(crate) fn tooltip(&self, point: (i32, i32)) -> Option<CollapsedSidebarTooltip> {
        let expand = self.expand_rect();
        let (anchor, text) = if rect_contains(&expand, point) {
            if self.compact {
                return None;
            }
            (
                expand,
                lingxia_logic::i18n::t(lingxia_logic::I18nKey::SidebarExpand),
            )
        } else {
            let (index, anchor) = self
                .header_action_rects()
                .into_iter()
                .find(|(_, rect)| rect_contains(rect, point))?;
            (anchor, self.header_actions[index].label.clone())
        };
        tooltip_below(
            anchor,
            &text,
            offset_rect(self.client, -self.frame.left, -self.frame.top),
        )
    }

    /// Horizontal offset of the content beyond the rail column at `progress`
    /// (0..=1). It starts fully tucked behind the rail column, which stays put.
    pub(crate) fn slide_shift(&self, progress: f32) -> i32 {
        let travel = (rect_width(&self.body) - rect_width(&self.rail)).max(0) as f32;
        let remaining = 1.0 - progress.clamp(0.0, 1.0);
        let offset = (remaining * remaining * remaining * travel).round() as i32;
        match self.position {
            WindowsShellTabBarPosition::Right => offset,
            _ => -offset,
        }
    }
}

fn offset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right + dx,
        bottom: rect.bottom + dy,
    }
}

/// Paints the peek into a frame-sized DC; `cursor` is in peek coordinates.
pub(crate) fn paint_sidebar_peek(hdc: HDC, peek: &SidebarPeekLayout, cursor: Option<(i32, i32)>) {
    let body = peek.local_body();
    let tabbar = peek.tabbar();
    let (scroll_offset, _, viewport_bottom) =
        sidebar_scroll_metrics(body, &peek.layout).unwrap_or((0, 0, body.bottom));
    draw_tab_bar(hdc, body, tabbar, cursor, scroll_offset, viewport_bottom);
    draw_footer_actions(hdc, body, &peek.local_rects(), &peek.layout, cursor);
    // Same metrics as the rail draws it, so opening the peek moves nothing.
    let expand = peek.expand_rect();
    draw_hover_wash(hdc, expand, 6, cursor);
    draw_design_icon_button(
        hdc,
        expand,
        WindowsDesignIcon::SidebarExpand,
        shell_palette().text_muted,
        sidebar_rail_icon_size(),
    );
    for (index, rect) in peek.header_action_rects() {
        let action = &peek.header_actions[index];
        if !action.disabled {
            draw_hover_wash(hdc, rect, 6, cursor);
        }
        let icon_rect = centered_icon_rect(rect, px(16));
        let _ = draw_icon_from_path(hdc, &action.icon_path, icon_rect, px(16) as u32);
    }
    match peek.position {
        WindowsShellTabBarPosition::Right => draw_left_border(hdc, body, shell_palette().divider),
        _ => draw_right_border(hdc, body, shell_palette().divider),
    }
}

/// Turns the painted frame into premultiplied ARGB: an opaque body, a soft
/// shadow toward the workspace, then the slide shift for everything outside
/// the rail column, which never moves.
pub(crate) fn compose_sidebar_peek_pixels(
    pixels: &mut [u32],
    peek: &SidebarPeekLayout,
    shift: i32,
) {
    let (width, height) = peek.size();
    if width <= 0 || height <= 0 || pixels.len() < (width * height) as usize {
        return;
    }
    let body = peek.local_body();
    let rail = offset_rect(peek.rail, -peek.frame.left, -peek.frame.top);
    let shadow = sidebar_peek_shadow().max(1);
    for y in 0..height {
        let row = &mut pixels[(y * width) as usize..((y + 1) * width) as usize];
        for (x, pixel) in row.iter_mut().enumerate() {
            let x = x as i32;
            *pixel = if rect_contains(&body, (x, y)) {
                0xff00_0000 | (*pixel & 0x00ff_ffff)
            } else {
                let distance = if x >= body.right {
                    x - body.right
                } else {
                    body.left - 1 - x
                };
                shadow_alpha(distance, shadow) << 24
            };
        }
        match peek.position {
            WindowsShellTabBarPosition::Right => {
                let end = rail.left.clamp(0, width) as usize;
                shift_row(&mut row[..end], shift);
            }
            _ => {
                let start = rail.right.clamp(0, width) as usize;
                shift_row(&mut row[start..], shift);
            }
        }
    }
}

fn shadow_alpha(distance: i32, shadow: i32) -> u32 {
    if distance < 0 || distance >= shadow {
        return 0;
    }
    let falloff = 1.0 - distance as f32 / shadow as f32;
    (SHADOW_MAX_ALPHA * falloff * falloff).round() as u32
}

fn shift_row(row: &mut [u32], shift: i32) {
    let len = row.len() as i32;
    if shift == 0 {
        return;
    }
    if shift.abs() >= len {
        row.fill(0);
        return;
    }
    let amount = shift.unsigned_abs() as usize;
    if shift > 0 {
        row.copy_within(..row.len() - amount, amount);
        row[..amount].fill(0);
    } else {
        row.copy_within(amount.., 0);
        let start = row.len() - amount;
        row[start..].fill(0);
    }
}

/// What Ctrl+\ does for this layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidebarShortcut {
    /// Pin or collapse, exactly like the sidebar's own toggle.
    Toggle,
    /// Compact desktop cannot pin, so the shortcut opens or closes the peek.
    Peek,
}

pub(crate) fn sidebar_shortcut(layout: &WindowsWindowLayout) -> Option<SidebarShortcut> {
    let tabbar = shell_layout(layout)?.tab_bar.as_ref()?;
    if !tabbar.visible
        || !matches!(
            tabbar.position,
            WindowsShellTabBarPosition::Left | WindowsShellTabBarPosition::Right
        )
    {
        return None;
    }
    Some(if tabbar.rail_expand_disabled {
        SidebarShortcut::Peek
    } else {
        SidebarShortcut::Toggle
    })
}

/// Choosing a destination closes the peek; toggles leave it open.
pub(crate) fn sidebar_peek_dismisses(command: &WindowsChromeCommand) -> bool {
    matches!(
        command.id.as_str(),
        command_id::TAB_BAR_CLICK
            | command_id::BROWSER_TAB_CLICK
            | command_id::BROWSER_NEW_TAB
            | command_id::MAIN_WORKSPACE_ADD
            | command_id::SIDEBAR_TOGGLE
    )
}

pub(crate) fn is_sidebar_peek_command(command: &WindowsChromeCommand) -> bool {
    command.id == command_id::SIDEBAR_PEEK
}

pub(crate) fn sidebar_toggle_command() -> WindowsChromeCommand {
    WindowsChromeCommand::new(command_id::SIDEBAR_TOGGLE).with_payload(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> RECT {
        RECT {
            left: 0,
            top: 0,
            right: 900,
            bottom: 700,
        }
    }

    fn rail_layout(position: WindowsShellTabBarPosition) -> WindowsShellWindowLayout {
        WindowsShellWindowLayout {
            tab_bar: Some(WindowsShellTabBarLayout {
                visible: true,
                position,
                dimension: 220,
                app_name: "App".to_string(),
                app_icon_path: String::new(),
                group_id: "app".to_string(),
                group_target_id: "lxapp:app".to_string(),
                group_active: true,
                group_closable: false,
                group_order_index: 0,
                color: 0,
                selected_color: 0,
                background_color: 0,
                paint_items_background: false,
                background_transparent: false,
                border_color: 0,
                selected_index: 0,
                overflow_start_index: -1,
                items: (0..3)
                    .map(|index| WindowsShellTabBarItemLayout {
                        index,
                        page_path: format!("page-{index}"),
                        text: format!("Page {index}"),
                        icon_path: String::new(),
                        badge: None,
                        has_red_dot: false,
                    })
                    .collect(),
                collapsed: false,
                icon_rail: true,
                rail_expand_disabled: false,
                items_api_hidden: false,
                items_collapsed: false,
                footer_action_height: 0,
                main_scroll_offset: 40,
                footer_action_scroll_row: 0,
                auxiliary_items: Vec::new(),
                show_auxiliary_add: false,
                header_actions: Vec::new(),
            }),
            footer_actions: vec![WindowsShellFooterActionLayout {
                generation: 1,
                id: "terminal".to_string(),
                label: "Terminal".to_string(),
                value: None,
                icon_path: String::new(),
                disabled: false,
                source: WindowsShellSidebarActionSource::Runtime,
            }],
            ..Default::default()
        }
    }

    fn center(rect: RECT) -> (i32, i32) {
        ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2)
    }

    fn command_id_of(hit: Option<WindowsChromeHit>) -> Option<String> {
        match hit? {
            WindowsChromeHit::Command(command)
            | WindowsChromeHit::CommandWithContext { command, .. } => Some(command.id),
            _ => None,
        }
    }

    #[test]
    fn peek_paints_the_expanded_sidebar_without_moving_the_workspace() {
        let layout = rail_layout(WindowsShellTabBarPosition::Left);
        let before = compute_chrome_rects(client(), &layout);
        let peek = peek_layout_for(client(), &layout).unwrap();
        let rail = before.tab_bar.unwrap();

        let tabbar = peek.tabbar();
        assert!(!tabbar.collapsed && !tabbar.icon_rail);
        assert_eq!(tabbar.main_scroll_offset, 0, "the peek scrolls on its own");
        assert_eq!(
            tabbar.footer_action_height,
            panel_footer_action_height(220, &layout.footer_actions)
        );
        assert_eq!(peek.body.left, rail.left);
        assert_eq!(rect_width(&peek.body), 220);
        assert_eq!(peek.frame.right, peek.body.right + sidebar_peek_shadow());
        // The real layout is untouched: still a rail, same workspace.
        assert!(layout.tab_bar.as_ref().unwrap().icon_rail);
        assert_eq!(
            compute_chrome_rects(client(), &layout).content,
            before.content
        );
    }

    #[test]
    fn right_peek_grows_toward_the_workspace_on_the_left() {
        let layout = rail_layout(WindowsShellTabBarPosition::Right);
        let rail = compute_chrome_rects(client(), &layout).tab_bar.unwrap();
        let peek = peek_layout_for(client(), &layout).unwrap();
        assert_eq!(peek.body.right, rail.right);
        assert_eq!(peek.body.top, rail.top);
        assert_eq!(peek.frame.left, peek.body.left - sidebar_peek_shadow());
        assert!(peek.slide_shift(0.0) > 0);
    }

    #[test]
    fn no_peek_without_a_rail() {
        let mut layout = rail_layout(WindowsShellTabBarPosition::Left);
        layout.tab_bar.as_mut().unwrap().icon_rail = false;
        assert!(peek_layout_for(client(), &layout).is_none());
        layout.tab_bar.as_mut().unwrap().position = WindowsShellTabBarPosition::Bottom;
        assert!(peek_layout_for(client(), &layout).is_none());
    }

    #[test]
    fn peek_hits_route_through_the_expanded_sidebar() {
        let layout = rail_layout(WindowsShellTabBarPosition::Left);
        let peek = peek_layout_for(client(), &layout).unwrap();
        let body = peek.local_body();

        let row = sidebar_item_rect(body, peek.tabbar(), 1, 0);
        assert!(matches!(
            peek.hit(center(row)),
            Some(WindowsChromeHit::Command(command))
                if command.id == command_id::TAB_BAR_CLICK && command.payload["index"] == 1
        ));
        let expand = peek.expand_rect();
        // No toggle in the collapse slot; the rail's control is the pin.
        assert!(matches!(
            peek.hit(center(sidebar_header_toggle_rect(body))),
            Some(WindowsChromeHit::Chrome)
        ));
        assert_eq!(
            command_id_of(peek.hit(center(expand))).as_deref(),
            Some(command_id::SIDEBAR_TOGGLE)
        );
        let footer = footer_action_rects(body, &peek.local_rects(), &peek.layout)[0].1;
        assert_eq!(
            command_id_of(peek.hit(center(footer))).as_deref(),
            Some(command_id::FOOTER_ACTION_CLICK)
        );
        // The drag strip is inert, the shadow is outside.
        assert!(matches!(
            peek.hit((body.left + 2, body.top + 2)),
            Some(WindowsChromeHit::Chrome)
        ));
        assert!(peek.hit((body.right + 2, center(body).1)).is_none());
    }

    #[test]
    fn peek_keeps_the_rail_expand_control_in_place() {
        for position in [
            WindowsShellTabBarPosition::Left,
            WindowsShellTabBarPosition::Right,
        ] {
            let layout = rail_layout(position);
            let rail = compute_chrome_rects(client(), &layout).tab_bar.unwrap();
            let peek = peek_layout_for(client(), &layout).unwrap();
            assert_eq!(
                offset_rect(peek.expand_rect(), peek.frame.left, peek.frame.top),
                sidebar_rail_expand_rect(rail),
                "{position:?}"
            );
            // Expanded content starts below the control row, as in the rail.
            let group = sidebar_group_rect(peek.local_body(), peek.tabbar(), 0);
            assert!(group.top >= peek.expand_rect().bottom);
        }
    }

    #[test]
    fn header_actions_keep_pinned_places_but_clear_the_expand_control() {
        let action = |id: &str| WindowsShellHeaderActionLayout {
            generation: 3,
            id: id.to_string(),
            label: id.to_string(),
            icon_path: String::new(),
            disabled: false,
            source: WindowsShellSidebarActionSource::Runtime,
        };
        for position in [
            WindowsShellTabBarPosition::Left,
            WindowsShellTabBarPosition::Right,
        ] {
            let mut layout = rail_layout(position);
            layout.tab_bar.as_mut().unwrap().header_actions =
                vec![action("settings"), action("downloads")];
            let peek = peek_layout_for(client(), &layout).unwrap();
            // Where the pinned expanded sidebar draws them, in peek coordinates.
            let mut pinned = layout.clone();
            pinned.tab_bar.as_mut().unwrap().icon_rail = false;
            let pinned_rect = compute_chrome_rects(client(), &pinned).tab_bar.unwrap();
            let mut expected =
                sidebar_header_action_rects(pinned_rect, pinned.tab_bar.as_ref().unwrap())
                    .into_iter()
                    .map(|(_, rect)| offset_rect(rect, -peek.frame.left, -peek.frame.top))
                    .collect::<Vec<_>>();
            expected.sort_by_key(|rect| rect.left);
            let mut rects = peek.header_action_rects();
            rects.sort_by_key(|(index, _)| *index);
            assert_eq!(rects.len(), 2, "{position:?}");
            assert!(rects[0].1.right <= rects[1].1.left, "declaration order");
            let expand = peek.expand_rect();
            for (_, rect) in &rects {
                assert!(
                    rect.right + sidebar_header_action_gap() <= expand.left
                        || rect.left >= expand.right,
                    "{position:?} overlaps the expand control"
                );
                assert!(rect.bottom <= shell_top_bar_height() + peek.local_body().top);
            }
            let actual = rects.iter().map(|(_, rect)| *rect).collect::<Vec<_>>();
            match position {
                // The rail control sits at the leading edge: pinned places hold.
                WindowsShellTabBarPosition::Left => assert_eq!(actual, expected),
                // The trailing action would run under the rail control; the
                // run moves left just enough, keeping its row and spacing.
                _ => {
                    assert_eq!(actual[1].right, expand.left - sidebar_header_action_gap());
                    let delta = expected[1].left - actual[1].left;
                    assert!(delta > 0);
                    assert_eq!(actual[0], offset_rect(expected[0], -delta, 0));
                }
            }
            let group = sidebar_group_rect(peek.local_body(), peek.tabbar(), 0);
            assert!(group.top >= expand.bottom);
            assert!(matches!(
                peek.hit(center(rects[0].1)),
                Some(WindowsChromeHit::Command(command))
                    if command.id == command_id::SIDEBAR_ACTION
                        && command.payload["action_id"] == "settings"
            ));
            assert_eq!(peek.hover_rect(center(rects[1].1)), Some(rects[1].1));
        }
    }

    #[test]
    fn peek_tooltips_name_the_control_under_the_pointer() {
        let action = |id: &str, label: &str| WindowsShellHeaderActionLayout {
            generation: 1,
            id: id.to_string(),
            label: label.to_string(),
            icon_path: String::new(),
            disabled: false,
            source: WindowsShellSidebarActionSource::Runtime,
        };
        for position in [
            WindowsShellTabBarPosition::Left,
            WindowsShellTabBarPosition::Right,
        ] {
            let mut layout = rail_layout(position);
            layout.tab_bar.as_mut().unwrap().header_actions = vec![
                action("settings", "Settings"),
                action("downloads", "Downloads"),
            ];
            let peek = peek_layout_for(client(), &layout).unwrap();
            let window_client = offset_rect(client(), -peek.frame.left, -peek.frame.top);
            let expand = peek.expand_rect();
            let tooltip = peek.tooltip(center(expand)).unwrap();
            assert_eq!(
                tooltip.text,
                lingxia_logic::i18n::t(lingxia_logic::I18nKey::SidebarExpand)
            );
            assert_eq!(tooltip.anchor, expand);
            assert!(
                tooltip.delayed,
                "a control label waits for the system delay"
            );
            for (index, rect) in peek.header_action_rects() {
                let tooltip = peek.tooltip(center(rect)).unwrap();
                assert_eq!(
                    tooltip.text, peek.header_actions[index].label,
                    "{position:?}"
                );
                assert_eq!(tooltip.anchor, rect);
                assert!(tooltip.delayed);
                assert!(
                    tooltip.popup.top >= rect.bottom,
                    "anchored under the button"
                );
                assert!(tooltip.popup.left >= window_client.left);
                assert!(tooltip.popup.right <= window_client.right, "{position:?}");
            }
            // Nothing to name over the rows' blank caption space.
            assert!(
                peek.tooltip((peek.local_body().left + 2, peek.local_body().top + 2))
                    .is_none()
            );
        }
    }

    #[test]
    fn compact_expand_control_closes_instead_of_pinning() {
        let mut layout = rail_layout(WindowsShellTabBarPosition::Left);
        layout.tab_bar.as_mut().unwrap().rail_expand_disabled = true;
        let peek = peek_layout_for(client(), &layout).unwrap();
        assert!(peek.compact);
        assert!(peek.tooltip(center(peek.expand_rect())).is_none());
        assert_eq!(
            command_id_of(peek.hit(center(peek.expand_rect()))).as_deref(),
            Some(command_id::SIDEBAR_PEEK)
        );
    }

    #[test]
    fn shortcut_peeks_only_where_pinning_is_unavailable() {
        let mut layout = rail_layout(WindowsShellTabBarPosition::Left);
        assert_eq!(
            sidebar_shortcut(&WindowsWindowLayout::new(layout.clone())),
            Some(SidebarShortcut::Toggle)
        );
        layout.tab_bar.as_mut().unwrap().rail_expand_disabled = true;
        assert_eq!(
            sidebar_shortcut(&WindowsWindowLayout::new(layout.clone())),
            Some(SidebarShortcut::Peek)
        );
        layout.tab_bar.as_mut().unwrap().position = WindowsShellTabBarPosition::Bottom;
        assert_eq!(sidebar_shortcut(&WindowsWindowLayout::new(layout)), None);
    }

    #[test]
    fn navigation_dismisses_and_toggles_keep_the_peek() {
        let command = |id: &str| WindowsChromeCommand::new(id);
        for id in [
            command_id::TAB_BAR_CLICK,
            command_id::BROWSER_TAB_CLICK,
            command_id::BROWSER_NEW_TAB,
            command_id::MAIN_WORKSPACE_ADD,
            command_id::SIDEBAR_TOGGLE,
        ] {
            assert!(sidebar_peek_dismisses(&command(id)), "{id}");
        }
        for id in [
            command_id::SIDEBAR_GROUP_TOGGLE,
            command_id::BROWSER_TAB_CLOSE,
            command_id::SIDEBAR_AUXILIARY_CONTEXT_MENU,
            command_id::FOOTER_ACTION_CLICK,
            command_id::STATIC_SETTINGS_CLICK,
            command_id::SIDEBAR_ACTION,
        ] {
            assert!(!sidebar_peek_dismisses(&command(id)), "{id}");
        }
    }

    #[test]
    fn wheel_scrolls_only_the_peek_and_survives_a_refresh() {
        let mut layout = rail_layout(WindowsShellTabBarPosition::Left);
        layout.tab_bar.as_mut().unwrap().items = (0..30)
            .map(|index| WindowsShellTabBarItemLayout {
                index,
                page_path: format!("page-{index}"),
                text: format!("Page {index}"),
                icon_path: String::new(),
                badge: None,
                has_red_dot: false,
            })
            .collect();
        let mut peek = peek_layout_for(client(), &layout).unwrap();
        let row = sidebar_item_rect(peek.local_body(), peek.tabbar(), 2, 0);
        assert!(peek.wheel(center(row), -120));
        let scrolled = peek.tabbar().main_scroll_offset;
        assert!(scrolled > 0);
        assert_eq!(layout.tab_bar.as_ref().unwrap().main_scroll_offset, 40);

        let next = peek_layout_for(client(), &layout).unwrap();
        assert!(peek.same_placement(&next));
        peek.refresh_from(next);
        assert_eq!(peek.tabbar().main_scroll_offset, scrolled);
    }

    #[test]
    fn slide_starts_at_the_rail_edge_and_settles_in_place() {
        let peek =
            peek_layout_for(client(), &rail_layout(WindowsShellTabBarPosition::Left)).unwrap();
        assert_eq!(
            peek.slide_shift(0.0),
            -(rect_width(&peek.body) - shell_sidebar_rail_width())
        );
        assert_eq!(peek.slide_shift(1.0), 0);
        assert!(peek.slide_shift(0.5) > peek.slide_shift(0.0));
    }

    #[test]
    fn composed_frame_is_opaque_body_and_fading_shadow() {
        let peek =
            peek_layout_for(client(), &rail_layout(WindowsShellTabBarPosition::Left)).unwrap();
        let (width, height) = peek.size();
        let mut pixels = vec![0x0012_3456; (width * height) as usize];
        compose_sidebar_peek_pixels(&mut pixels, &peek, 0);
        let at = |x: i32, y: i32| pixels[(y * width + x) as usize];
        let body = peek.local_body();
        assert_eq!(at(body.left + 3, 10), 0xff12_3456);
        let near = at(body.right, 10) >> 24;
        let far = at(body.right + sidebar_peek_shadow() - 1, 10) >> 24;
        assert!(near > far && near > 0);
        assert_eq!(
            at(body.right, 10) & 0x00ff_ffff,
            0,
            "shadow is premultiplied black"
        );

        // The rail column stays put while the rest slides out from under it.
        let mut painted = vec![0u32; (width * height) as usize];
        for (index, pixel) in painted.iter_mut().enumerate() {
            *pixel = (index as u32 % width as u32) & 0x00ff_ffff;
        }
        let mut shifted = painted.clone();
        compose_sidebar_peek_pixels(&mut shifted, &peek, -10);
        let rail_right = peek.rail.right - peek.frame.left;
        let row = (10 * width) as usize;
        for x in [0, rail_right - 1] {
            assert_eq!(
                shifted[row + x as usize] & 0x00ff_ffff,
                x as u32,
                "rail x={x}"
            );
        }
        assert_eq!(
            shifted[row + rail_right as usize] & 0x00ff_ffff,
            (rail_right + 10) as u32
        );
        assert_eq!(shifted[row + width as usize - 1], 0);
    }

    #[test]
    fn right_slide_keeps_the_rail_column_fixed() {
        let peek =
            peek_layout_for(client(), &rail_layout(WindowsShellTabBarPosition::Right)).unwrap();
        let (width, height) = peek.size();
        let mut pixels = vec![0u32; (width * height) as usize];
        for (index, pixel) in pixels.iter_mut().enumerate() {
            *pixel = index as u32 % width as u32;
        }
        compose_sidebar_peek_pixels(&mut pixels, &peek, 10);
        let rail_left = peek.rail.left - peek.frame.left;
        let row = (10 * width) as usize;
        assert_eq!(
            pixels[row + rail_left as usize] & 0x00ff_ffff,
            rail_left as u32
        );
        assert_eq!(
            pixels[row + rail_left as usize - 1] & 0x00ff_ffff,
            (rail_left - 11) as u32
        );
        assert_eq!(pixels[row], 0);
    }
}
