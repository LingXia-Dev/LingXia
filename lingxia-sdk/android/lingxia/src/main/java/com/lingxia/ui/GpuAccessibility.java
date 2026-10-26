package com.lingxia.ui;

import android.graphics.Rect;
import android.os.Build;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import com.lingxia.ui.generated.GpuSemantics;
import org.json.JSONArray;
import org.json.JSONObject;

/** Maps the shared semantic projection onto AccessibilityNodeInfo; no role or state inference here. */
final class GpuAccessibility {
    static final int SELECT_ITEM = 0x02010000;
    static final int SET_PROGRESS = 16908349;
    /** AndroidX `AccessibilityNodeInfoCompat.setRoleDescription` key, read by TalkBack. */
    static final String ROLE_DESCRIPTION = "AccessibilityNodeInfo.roleDescription";

    // Rust action opcodes; see the JNI accessibility codec.
    static final int FOCUS = 0, CLEAR_FOCUS = 1, CLICK = 2, SHOW_ON_SCREEN = 3, SCROLL_FORWARD = 4, SCROLL_BACKWARD = 5,
        PROGRESS = 6, SET_TEXT = 7, SET_SELECTION = 8, CLEAR_SELECTION = 9, SELECT = 10;

    static String className(String role) {
        switch (role) {
            case GpuSemantics.ROLE_BUTTON: return "android.widget.Button";
            case GpuSemantics.ROLE_CHECK_BOX: return "android.widget.CheckBox";
            case GpuSemantics.ROLE_SWITCH: return "android.widget.Switch";
            case GpuSemantics.ROLE_RADIO_GROUP: return "android.widget.RadioGroup";
            case GpuSemantics.ROLE_RADIO_BUTTON: return "android.widget.RadioButton";
            case GpuSemantics.ROLE_TEXT_FIELD: return "android.widget.EditText";
            case GpuSemantics.ROLE_SCROLL_VIEW: return "android.widget.ScrollView";
            case GpuSemantics.ROLE_HORIZONTAL_SCROLL_VIEW: return "android.widget.HorizontalScrollView";
            case GpuSemantics.ROLE_IMAGE: return "android.widget.ImageView";
            case GpuSemantics.ROLE_SLIDER: return "android.widget.SeekBar";
            case GpuSemantics.ROLE_LIST: return "android.widget.ListView";
            // Links (with a role description), headings, list items, alerts, status and dialogs read as text plus their traits.
            default: return "android.widget.TextView";
        }
    }

    static boolean hasAction(JSONObject node, String kind) {
        JSONArray actions = node.optJSONArray("actions");
        for (int i = 0; actions != null && i < actions.length(); i++)
            if (kind.equals(actions.optJSONObject(i).optString("kind"))) return true;
        return false;
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
        // TalkBack speaks this in place of the class role; a link has no widget class of its own.
        if (GpuSemantics.ROLE_LINK.equals(role))
            info.getExtras().putCharSequence(ROLE_DESCRIPTION, host.getContext().getString(com.lingxia.lxapp.R.string.lx_gpu_a11y_link));
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
        JSONArray selected = node.optJSONArray(GpuSemantics.KEY_TEXT_SELECTION);
        if (field == null && selected != null) info.setTextSelection(selected.optInt(0), selected.optInt(1));
        JSONArray position = node.optJSONArray(GpuSemantics.KEY_POSITION);
        if (position != null && GpuSemantics.ROLE_RADIO_GROUP.equals(role)) {
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain(position.optInt(1), 1, false,
                AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE));
        } else if (position != null && GpuSemantics.ROLE_RADIO_BUTTON.equals(role)) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(position.optInt(0), 1, 0, 1, false, node.optBoolean("checked")));
        } else if (position != null && GpuSemantics.ROLE_LIST.equals(role)) {
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain(position.optInt(1), 1, false));
        } else if (position != null && GpuSemantics.ROLE_LIST_ITEM.equals(role)) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(position.optInt(0), 1, 0, 1, false));
        }
        if (GpuSemantics.ROLE_HEADING.equals(role)) {
            if (Build.VERSION.SDK_INT >= 28) info.setHeading(true);
            else info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(0, 1, 0, 1, true));
        }
        if (GpuSemantics.ROLE_DIALOG.equals(role)) {
            if (Build.VERSION.SDK_INT >= 28 && !name.isEmpty()) info.setPaneTitle(name);
            info.setClickable(false);
        }
        String live = node.optString(GpuSemantics.KEY_LIVE_REGION);
        if (GpuSemantics.LIVE_POLITE.equals(live)) info.setLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        else if (GpuSemantics.LIVE_ASSERTIVE.equals(live)) info.setLiveRegion(View.ACCESSIBILITY_LIVE_REGION_ASSERTIVE);
        JSONArray actions = node.optJSONArray("actions");
        for (int i = 0; actions != null && i < actions.length(); i++) {
            JSONObject action = actions.optJSONObject(i);
            switch (action.optString("kind")) {
                case "click": info.addAction(AccessibilityNodeInfo.ACTION_CLICK); break;
                case "scroll-forward": info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD); break;
                case "scroll-backward": info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD); break;
                case "set-text": info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT); break;
                case "set-selection": info.addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION); break;
                case GpuSemantics.ACTION_COPY: info.addAction(AccessibilityNodeInfo.ACTION_COPY); break;
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
