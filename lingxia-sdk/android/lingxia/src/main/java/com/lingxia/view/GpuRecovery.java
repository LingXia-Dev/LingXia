package com.lingxia.view;

import android.content.res.Configuration;
import android.graphics.Color;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import com.lingxia.lxapp.R;

/** Host-owned feedback; author content cannot replace its recovery action. The look matches
 *  every platform's: the message centred, then text buttons in the accent colour, on the page's
 *  own canvas. */
final class GpuRecovery {
    /** The shared host's recovery messages, by code. */
    private static final int[] MESSAGES = {R.string.lx_gpu_update_required, R.string.lx_gpu_session_failed,
        R.string.lx_gpu_update_failed, R.string.lx_gpu_display_failed, R.string.lx_gpu_slow_load};
    /** Its actions: 0 retry, 1 reload, 2 keep waiting. */
    private static final int[] ACTIONS = {R.string.lx_gpu_retry, R.string.lx_gpu_reload, R.string.lx_gpu_keep_waiting};
    private static final int KEEP_WAITING = 2;
    private final FrameLayout host;
    private LinearLayout panel;
    private boolean banner;
    private TextView message;
    private Button action, back;
    private final android.graphics.Rect insets = new android.graphics.Rect();
    /** The page's canvas, text and accent colours, once the core has resolved its palette. */
    private Integer canvas, text, accent;

    GpuRecovery(FrameLayout host) { this.host = host; }

    /** {@code back}, when non-null, offers Back beside the action; false re-enables it. */
    void show(boolean retained, int code, int actionCode, Runnable retry, java.util.function.BooleanSupplier back) {
        clear();
        android.content.Context context = host.getContext();
        float density = host.getResources().getDisplayMetrics().density;
        LinearLayout current = new LinearLayout(context);
        panel = current;
        current.setOrientation(LinearLayout.VERTICAL);
        current.setGravity(Gravity.CENTER);
        banner = retained;
        current.setClickable(true);
        current.setTag(GpuView.RECOVERY_TAG);
        TextView label = new TextView(context);
        message = label;
        label.setTextSize(17);
        label.setGravity(Gravity.CENTER);
        label.setText(string(MESSAGES[code]));
        label.setAccessibilityLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        current.addView(label, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT));
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER);
        this.back = null;
        this.action = null;
        if (back != null) {
            Button button = textButton(context, R.string.lx_gpu_back);
            this.back = button;
            button.setOnClickListener(view -> {
                if (panel != current || !button.isEnabled()) return;
                button.setEnabled(false);
                if (!back.getAsBoolean()) button.setEnabled(true);
            });
            row.addView(button);
        }
        if (actionCode >= 0 && retry != null) {
            Button button = textButton(context, ACTIONS[actionCode]);
            this.action = button;
            button.setOnClickListener(view -> {
                if (panel != current || !button.isEnabled()) return;
                button.setEnabled(false);
                if (actionCode != KEEP_WAITING) button.setText(string(R.string.lx_gpu_retrying));
                retry.run();
            });
            row.addView(button);
        }
        if (row.getChildCount() > 0) {
            LinearLayout.LayoutParams layout = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT);
            layout.topMargin = Math.round(4 * density);
            current.addView(row, layout);
        }
        FrameLayout.LayoutParams layout = new FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT,
            retained ? FrameLayout.LayoutParams.WRAP_CONTENT : FrameLayout.LayoutParams.MATCH_PARENT);
        if (retained) layout.gravity = Gravity.BOTTOM;
        host.addView(current, layout);
        place();
        paint();
    }

    /** The app's display language, which can differ from the system's, as the page uses. */
    private String string(int id) {
        return com.lingxia.app.Lingxia.localizedString(host.getContext(), id);
    }

    /** A borderless label button, as iOS's system buttons: no plate, no shadow, no capitals. */
    private Button textButton(android.content.Context context, int title) {
        Button button = new Button(context, null, android.R.attr.borderlessButtonStyle);
        button.setText(string(title));
        button.setAllCaps(false);
        // Message 17, buttons 16 medium: the panel reads the same on every platform.
        button.setTextSize(16);
        button.setTypeface(android.graphics.Typeface.create("sans-serif-medium", android.graphics.Typeface.NORMAL));
        button.setStateListAnimator(null);
        float density = context.getResources().getDisplayMetrics().density;
        int side = Math.round(16 * density);
        button.setPadding(side, button.getPaddingTop(), side, button.getPaddingBottom());
        int target = Math.round(48 * density);
        button.setMinHeight(target);
        button.setMinimumHeight(target);
        button.setMinWidth(target);
        button.setMinimumWidth(target);
        return button;
    }

    /** The page's palette: the panel sits on its canvas, as the page would. */
    void setColors(int canvas, int text, int accent) {
        this.canvas = canvas;
        this.text = text;
        this.accent = accent;
        paint();
    }

    private void paint() {
        if (panel == null) return;
        boolean dark = (host.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
                == Configuration.UI_MODE_NIGHT_YES;
        panel.setBackgroundColor(canvas != null ? canvas : dark ? Color.rgb(32, 34, 38) : Color.WHITE);
        message.setTextColor(text != null ? text : dark ? Color.WHITE : Color.rgb(32, 34, 38));
        int tint = accent != null ? accent : Color.rgb(0, 122, 255);
        for (Button button : new Button[] {back, action}) if (button != null) button.setTextColor(tint);
    }

    TextView message() { return message; }
    Button action() { return action; }
    Button back() { return back; }

    void setInsets(android.graphics.Rect value) {
        insets.set(value);
        place();
    }

    /** A banner's plate runs to the bottom edge, its buttons above the bottom inset. */
    private void place() {
        if (panel == null) return;
        int padding = Math.round(16 * host.getResources().getDisplayMetrics().density);
        panel.setPadding(padding, padding, padding, padding + (banner ? insets.bottom : 0));
        FrameLayout.LayoutParams layout = (FrameLayout.LayoutParams)panel.getLayoutParams();
        layout.setMargins(insets.left, insets.top, insets.right, banner ? 0 : insets.bottom);
        panel.setLayoutParams(layout);
    }

    void clear() {
        if (panel != null) { host.removeView(panel); panel = null; message = null; action = null; back = null; }
    }
}
