package com.lingxia.ui;

import android.content.res.Configuration;
import android.graphics.Color;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import com.lingxia.lxapp.R;

/** Host-owned feedback; author content cannot replace its recovery action. */
final class GpuRecovery {
    enum Kind { CANDIDATE, COMPATIBILITY, SESSION }
    static Kind kind(Throwable error) {
        String message = error.toString();
        return message.contains("UI_COMPAT") ? Kind.COMPATIBILITY
                : message.contains("UI_TIMEOUT:") ? Kind.SESSION : Kind.CANDIDATE;
    }
    private final FrameLayout host;
    private LinearLayout panel;
    private final android.graphics.Rect insets = new android.graphics.Rect();

    GpuRecovery(FrameLayout host) { this.host = host; }

    void show(boolean retained, Kind kind, Runnable retry) {
        clear();
        LinearLayout current = new LinearLayout(host.getContext());
        panel = current;
        current.setOrientation(LinearLayout.VERTICAL);
        boolean dark = (host.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
                == Configuration.UI_MODE_NIGHT_YES;
        current.setBackgroundColor(dark ? Color.rgb(32, 34, 38) : Color.WHITE);
        int padding = Math.round(16 * host.getResources().getDisplayMetrics().density);
        current.setPadding(padding, padding, padding, padding);
        current.setClickable(true);
        TextView message = new TextView(host.getContext());
        message.setTextColor(dark ? Color.WHITE : Color.rgb(32, 34, 38));
        message.setTextSize(16);
        message.setText(kind == Kind.COMPATIBILITY ? R.string.lx_gpu_update_required
                : kind == Kind.SESSION ? R.string.lx_gpu_session_failed
                : retained ? R.string.lx_gpu_update_failed : R.string.lx_gpu_display_failed);
        message.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        current.addView(message);
        if (kind != Kind.COMPATIBILITY && retry != null) {
            Button button = new Button(host.getContext());
            button.setText(kind == Kind.SESSION ? R.string.lx_gpu_reload : R.string.lx_gpu_retry);
            button.setOnClickListener(view -> {
                if (panel != current || !button.isEnabled()) return;
                button.setEnabled(false);
                button.setText(R.string.lx_gpu_retrying);
                retry.run();
            });
            current.addView(button);
        }
        FrameLayout.LayoutParams layout = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
                retained ? FrameLayout.LayoutParams.WRAP_CONTENT : FrameLayout.LayoutParams.MATCH_PARENT);
        if (retained) layout.gravity = android.view.Gravity.BOTTOM;
        layout.setMargins(insets.left, insets.top, insets.right, insets.bottom);
        host.addView(current, layout);
    }

    void setInsets(android.graphics.Rect value) {
        insets.set(value);
        if (panel != null) {
            FrameLayout.LayoutParams layout = (FrameLayout.LayoutParams)panel.getLayoutParams();
            layout.setMargins(insets.left, insets.top, insets.right, insets.bottom);
            panel.setLayoutParams(layout);
        }
    }

    void clear() {
        if (panel != null) { host.removeView(panel); panel = null; }
    }
}
