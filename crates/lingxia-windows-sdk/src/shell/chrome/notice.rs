//! Short-lived shell-owned notices shown above the WebView layer.

use super::*;

const NOTICE_RADIUS: i32 = 12;
const NOTICE_BORDER: i32 = 1;
const NOTICE_ICON_SIZE: i32 = 30;
const NOTICE_PADDING: i32 = 14;

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
    fill_round_rect_aa(hdc, bounds, NOTICE_RADIUS, palette.divider);
    fill_round_rect_aa(
        hdc,
        RECT {
            left: NOTICE_BORDER,
            top: NOTICE_BORDER,
            right: width - NOTICE_BORDER,
            bottom: height - NOTICE_BORDER,
        },
        NOTICE_RADIUS - NOTICE_BORDER,
        palette.panel_background,
    );

    let icon = RECT {
        left: NOTICE_PADDING,
        top: (height - NOTICE_ICON_SIZE) / 2,
        right: NOTICE_PADDING + NOTICE_ICON_SIZE,
        bottom: (height + NOTICE_ICON_SIZE) / 2,
    };
    fill_round_rect_aa(hdc, icon, NOTICE_ICON_SIZE / 2, palette.control_surface);
    draw_text_antialiased(hdc, "!", icon, palette.accent, DT_CENTER);

    let text_left = icon.right + 12;
    let text_right = (width - NOTICE_PADDING).max(text_left);
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
            bottom: height - 9,
        },
        palette.text_muted,
    );
}
