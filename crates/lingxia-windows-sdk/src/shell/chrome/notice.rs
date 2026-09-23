//! Short-lived shell-owned notices shown above the WebView layer.

use super::*;
use crate::dpi::px;

fn notice_radius() -> i32 {
    crate::dpi::px(12)
}
fn notice_border() -> i32 {
    crate::dpi::px(1)
}
fn notice_icon_size() -> i32 {
    crate::dpi::px(30)
}
fn notice_padding() -> i32 {
    crate::dpi::px(14)
}

pub(crate) fn paint_shell_notice(hdc: HDC, title: &str, message: &str, width: i32, height: i32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let palette = shell_palette();
    let bounds = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    fill_round_rect_aa(hdc, bounds, notice_radius(), palette.divider);
    fill_round_rect_aa(
        hdc,
        RECT {
            left: notice_border(),
            top: notice_border(),
            right: width - notice_border(),
            bottom: height - notice_border(),
        },
        notice_radius() - notice_border(),
        palette.panel_background,
    );

    let icon = RECT {
        left: notice_padding(),
        top: (height - notice_icon_size()) / 2,
        right: notice_padding() + notice_icon_size(),
        bottom: (height + notice_icon_size()) / 2,
    };
    fill_round_rect_aa(hdc, icon, notice_icon_size() / 2, palette.control_surface);
    draw_text_antialiased(hdc, "!", icon, palette.accent, DT_CENTER);

    let text_left = icon.right + px(12);
    let text_right = (width - notice_padding()).max(text_left);
    draw_text_antialiased(
        hdc,
        title,
        RECT {
            left: text_left,
            top: 9,
            right: text_right,
            bottom: height / 2,
        },
        palette.text_primary,
        DT_LEFT,
    );
    draw_text_multiline_antialiased(
        hdc,
        message,
        RECT {
            left: text_left,
            top: 36,
            right: text_right,
            bottom: height - px(9),
        },
        palette.text_muted,
    );
}
