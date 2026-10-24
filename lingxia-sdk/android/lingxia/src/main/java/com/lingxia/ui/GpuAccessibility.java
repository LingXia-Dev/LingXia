package com.lingxia.ui;

import android.graphics.Rect;
import android.os.Build;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** Maps the shared semantic projection onto AccessibilityNodeInfo; no role or state inference here. */
final class GpuAccessibility {
    static final int SELECT_ITEM = 0x02010000;
    static final int SET_PROGRESS = 16908349;

    // Rust action opcodes; see the JNI accessibility codec.
    static final int FOCUS = 0, CLEAR_FOCUS = 1, CLICK = 2, SHOW_ON_SCREEN = 3, SCROLL_FORWARD = 4, SCROLL_BACKWARD = 5,
        PROGRESS = 6, SET_TEXT = 7, SET_SELECTION = 8, CLEAR_SELECTION = 9, SELECT = 10;

    static String className(String role) {
        switch (role) {
            case "button": return "android.widget.Button";
            case "check-box": return "android.widget.CheckBox";
            case "switch": return "android.widget.Switch";
            case "radio-group": return "android.widget.RadioGroup";
            case "radio-button": return "android.widget.RadioButton";
            case "text-field": return "android.widget.EditText";
            case "scroll-view": return "android.widget.ScrollView";
            case "horizontal-scroll-view": return "android.widget.HorizontalScrollView";
            case "image": return "android.widget.ImageView";
            case "slider": return "android.widget.SeekBar";
            default: return "android.widget.TextView";
        }
    }

    static Rect pixels(JSONArray bounds, float scale) {
        if (bounds == null) return new Rect();
        return new Rect((int)(bounds.optDouble(0) * scale), (int)(bounds.optDouble(1) * scale),
            (int)((bounds.optDouble(0) + bounds.optDouble(2)) * scale), (int)((bounds.optDouble(1) + bounds.optDouble(3)) * scale));
    }

    /** Populate `info` for `node`; `parent` is the containing group's projection, if any. */
    static void populate(AccessibilityNodeInfo info, View host, JSONObject node, JSONObject parent, float scale,
            boolean accessibilityFocused, String packageName) {
        String role = node.optString("role");
        info.setPackageName(packageName);
        info.setClassName(className(role));
        info.setText(node.optString("text"));
        String name = node.optString("name");
        if (!name.isEmpty() && !name.equals(node.optString("text"))) info.setContentDescription(name);
        info.setVisibleToUser(node.optBoolean("visible"));
        if (Build.VERSION.SDK_INT >= 24) info.setImportantForAccessibility(true);
        String testId = node.optString("test_id");
        if (!testId.isEmpty()) info.setViewIdResourceName(packageName + ":id/" + testId);
        info.setEnabled(node.optBoolean("enabled"));
        info.setFocusable(node.optBoolean("focusable"));
        info.setClickable(node.optBoolean("clickable"));
        info.setFocused(node.optBoolean("focused"));
        if (!node.isNull("checked") && node.has("checked")) { info.setCheckable(true); info.setChecked(node.optBoolean("checked")); }
        info.setScrollable(node.optBoolean("scrollable"));
        JSONObject range = node.optJSONObject("range");
        if (range != null) info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,
            (float)range.optDouble("min"), (float)range.optDouble("max"), (float)range.optDouble("value")));
        JSONObject field = node.optJSONObject("text_field");
        if (field != null) {
            info.setText(field.optString("text"));
            info.setEditable(true);
            info.setPassword(field.optBoolean("secure"));
            info.setMultiLine(field.optBoolean("multiline"));
            JSONArray selection = field.optJSONArray("selection");
            info.setTextSelection(selection == null ? -1 : selection.optInt(0, -1), selection == null ? -1 : selection.optInt(1, -1));
            if (Build.VERSION.SDK_INT >= 26) { info.setHintText(field.optString("hint")); info.setShowingHintText(false); }
        }
        JSONArray position = node.optJSONArray("position");
        if (position != null && "radio-group".equals(role)) {
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain(position.optInt(1), 1, false,
                AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE));
        } else if (position != null && "radio-button".equals(role)) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(position.optInt(0), 1, 0, 1, false, node.optBoolean("checked")));
        }
        JSONArray actions = node.optJSONArray("actions");
        for (int i = 0; actions != null && i < actions.length(); i++) {
            JSONObject action = actions.optJSONObject(i);
            switch (action.optString("kind")) {
                case "click": info.addAction(AccessibilityNodeInfo.ACTION_CLICK); break;
                case "scroll-forward": info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD); break;
                case "scroll-backward": info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD); break;
                case "set-text": info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT); break;
                case "set-selection": info.addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION); break;
                case "set-progress":
                    if (Build.VERSION.SDK_INT >= 24) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);
                    break;
                case "clear-selection": info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_SELECTION); break;
                case "select-item":
                    info.addAction(new AccessibilityNodeInfo.AccessibilityAction(SELECT_ITEM + action.optInt("index"), action.optString("label")));
                    break;
                case "show-on-screen": info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN); break;
                default: break;
            }
        }
        info.setAccessibilityFocused(accessibilityFocused);
        info.addAction(accessibilityFocused ? AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS : AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
        Rect rect = pixels(node.optJSONArray("bounds"), scale), relative = new Rect(rect);
        if (parent != null) {
            Rect group = pixels(parent.optJSONArray("bounds"), scale);
            relative.offset(-group.left, -group.top);
            info.setParent(host, parent.optInt("id"));
        } else {
            info.setParent(host);
        }
        JSONArray children = node.optJSONArray("children");
        for (int i = 0; children != null && i < children.length(); i++) info.addChild(host, children.optInt(i));
        info.setBoundsInParent(relative);
        int[] location = new int[2]; host.getLocationOnScreen(location); rect.offset(location[0], location[1]);
        info.setBoundsInScreen(rect);
    }

    /** Translate an Android action into the shared action opcode, or -1. */
    static int action(int action) {
        if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) return FOCUS;
        if (action == AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) return CLEAR_FOCUS;
        if (action == AccessibilityNodeInfo.ACTION_CLICK) return CLICK;
        if (action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()) return SHOW_ON_SCREEN;
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) return SCROLL_FORWARD;
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) return SCROLL_BACKWARD;
        if (action == SET_PROGRESS) return PROGRESS;
        if (action == AccessibilityNodeInfo.ACTION_SET_TEXT) return SET_TEXT;
        if (action == AccessibilityNodeInfo.ACTION_SET_SELECTION) return SET_SELECTION;
        if (action == AccessibilityNodeInfo.ACTION_CLEAR_SELECTION) return CLEAR_SELECTION;
        if (action >= SELECT_ITEM && action < SELECT_ITEM + 128) return SELECT;
        return -1;
    }
}
