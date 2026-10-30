package com.lingxia.ui;

import android.graphics.Rect;
import android.os.Build;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import com.lingxia.ui.generated.GpuFrame;
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
        PROGRESS = 6, SET_TEXT = 7, SET_SELECTION = 8, CLEAR_SELECTION = 9, SELECT = 10, COPY = 11, CUT = 12, PASTE = 13;

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
            case GpuSemantics.ROLE_COMBO_BOX: return "android.widget.Spinner";
            case GpuSemantics.ROLE_GRID: return "android.widget.GridView";
            case GpuSemantics.ROLE_GRID_CELL: return "android.widget.Button";
            case GpuSemantics.ROLE_LIST_BOX: return "android.widget.ListView";
            case GpuSemantics.ROLE_OPTION: return "android.widget.CheckedTextView";
            case GpuSemantics.ROLE_MENU: return "android.widget.ListView";
            case GpuSemantics.ROLE_MENU_ITEM: return "android.widget.Button";
            // Links (with a role description), headings, list items, alerts, status and dialogs read as text plus their traits.
            case GpuSemantics.ROLE_TEXT: case GpuSemantics.ROLE_LINK: case GpuSemantics.ROLE_HEADING:
            case GpuSemantics.ROLE_LIST_ITEM: case GpuSemantics.ROLE_DIALOG: case GpuSemantics.ROLE_ALERT:
            case GpuSemantics.ROLE_STATUS: return "android.widget.TextView";
            // A role the core added without a mapping here fails loudly, never reads as plain text.
            default: throw new IllegalArgumentException("unmapped semantic role " + role);
        }
    }

    static boolean hasAction(JSONObject node, String kind) {
        GpuFrame.SemanticAction[] actions = new GpuFrame.SemanticNode(node).actions();
        for (int i = 0; actions != null && i < actions.length; i++)
            if (kind.equals(actions[i].kind())) return true;
        return false;
    }

    static Rect pixels(float[] bounds, float scale) {
        if (bounds == null || bounds.length < 4) return new Rect();
        return new Rect((int)(bounds[0] * scale), (int)(bounds[1] * scale),
            (int)((bounds[0] + bounds[2]) * scale), (int)((bounds[1] + bounds[3]) * scale));
    }

    private static String text(String value) { return value == null ? "" : value; }

    /** Populate `info` for `node`; `parent` is the containing group's projection, if any. */
    static void populate(AccessibilityNodeInfo info, View host, JSONObject json, JSONObject parentJson, float scale,
            boolean accessibilityFocused, String packageName) {
        GpuFrame.SemanticNode node = new GpuFrame.SemanticNode(json);
        String role = text(node.role());
        info.setPackageName(packageName);
        info.setClassName(className(role));
        info.setText(text(node.text()));
        // TalkBack speaks this in place of the class role; a link has no widget class of its own.
        if (GpuSemantics.ROLE_LINK.equals(role))
            info.getExtras().putCharSequence(ROLE_DESCRIPTION, host.getContext().getString(com.lingxia.lxapp.R.string.lx_gpu_a11y_link));
        String name = text(node.name());
        // Android has no busy state; the spoken name carries it.
        if (Boolean.TRUE.equals(node.busy())) {
            String busy = host.getContext().getString(com.lingxia.lxapp.R.string.lx_gpu_a11y_busy);
            String spoken = name.isEmpty() ? text(node.text()) : name;
            name = spoken.isEmpty() ? busy : spoken + ", " + busy;
        }
        if (!name.isEmpty() && !name.equals(text(node.text()))) info.setContentDescription(name);
        // A select's chosen label (or placeholder) is its state, spoken after the name.
        String value = text(node.value());
        if (!value.isEmpty()) {
            if (Build.VERSION.SDK_INT >= 30) info.setStateDescription(value);
            else if (!name.isEmpty()) info.setContentDescription(name + ", " + value);
        }
        info.setVisibleToUser(node.visible());
        if (Build.VERSION.SDK_INT >= 24) info.setImportantForAccessibility(true);
        String testId = text(node.testId());
        if (!testId.isEmpty()) info.setViewIdResourceName(packageName + ":id/" + testId);
        info.setEnabled(node.enabled());
        info.setFocusable(node.focusable());
        info.setClickable(node.clickable());
        info.setFocused(node.focused());
        Boolean checked = node.checked(), pressed = node.pressed(), selectedState = node.selected(), expanded = node.expanded();
        if (checked != null) { info.setCheckable(true); info.setChecked(checked); }
        // A toggle button reads as checkable, the closest Android state to aria-pressed.
        else if (pressed != null) { info.setCheckable(true); info.setChecked(pressed); }
        if (selectedState != null) info.setSelected(selectedState);
        // Expanding is a click on the disclosure control; TalkBack announces the state from the action.
        if (expanded != null && node.clickable())
            info.addAction(expanded ? AccessibilityNodeInfo.ACTION_COLLAPSE : AccessibilityNodeInfo.ACTION_EXPAND);
        info.setScrollable(node.scrollable() || node.scroll() != null);
        GpuFrame.SemanticRange range = node.range();
        if (range != null) info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,
            (float)range.min(), (float)range.max(), (float)range.value()));
        GpuFrame.TextField field = node.textField();
        if (field != null) {
            info.setText(text(field.text()));
            info.setEditable(true);
            info.setPassword(field.secure());
            info.setMultiLine(field.multiline());
            long[] selection = field.selection();
            info.setTextSelection(selection == null || selection.length < 2 ? -1 : (int)selection[0],
                selection == null || selection.length < 2 ? -1 : (int)selection[1]);
            if (Build.VERSION.SDK_INT >= 26) { info.setHintText(text(field.hint())); info.setShowingHintText(false); }
        }
        long[] selected = node.textSelection();
        if (field == null && selected != null && selected.length >= 2) info.setTextSelection((int)selected[0], (int)selected[1]);
        long[] position = node.position();
        if (position != null && position.length < 2) position = null;
        if (position != null && GpuSemantics.ROLE_RADIO_GROUP.equals(role)) {
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain((int)position[1], 1, false,
                AccessibilityNodeInfo.CollectionInfo.SELECTION_MODE_SINGLE));
        } else if (position != null && GpuSemantics.ROLE_RADIO_BUTTON.equals(role)) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain((int)position[0], 1, 0, 1, false, Boolean.TRUE.equals(checked)));
        } else if (position != null && GpuSemantics.ROLE_LIST.equals(role)) {
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain((int)position[1], 1, false));
        } else if (position != null && GpuSemantics.ROLE_LIST_ITEM.equals(role)) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain((int)position[0], 1, 0, 1, false));
        }
        if (GpuSemantics.ROLE_HEADING.equals(role)) {
            if (Build.VERSION.SDK_INT >= 28) info.setHeading(true);
            else info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(0, 1, 0, 1, true));
        }
        if (GpuSemantics.ROLE_DIALOG.equals(role)) {
            if (Build.VERSION.SDK_INT >= 28 && !name.isEmpty()) info.setPaneTitle(name);
        }
        String live = node.liveRegion();
        if (GpuSemantics.LIVE_POLITE.equals(live)) info.setLiveRegion(View.ACCESSIBILITY_LIVE_REGION_POLITE);
        else if (GpuSemantics.LIVE_ASSERTIVE.equals(live)) info.setLiveRegion(View.ACCESSIBILITY_LIVE_REGION_ASSERTIVE);
        GpuFrame.SemanticAction[] actions = node.actions();
        for (int i = 0; actions != null && i < actions.length; i++) {
            GpuFrame.SemanticAction action = actions[i];
            switch (text(action.kind())) {
                case GpuFrame.SEMANTIC_ACTION_CLICK: info.addAction(AccessibilityNodeInfo.ACTION_CLICK); break;
                case GpuFrame.SEMANTIC_ACTION_SCROLL_FORWARD: info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD); break;
                case GpuFrame.SEMANTIC_ACTION_SCROLL_BACKWARD: info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD); break;
                case GpuFrame.SEMANTIC_ACTION_SET_TEXT: info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT); break;
                case GpuFrame.SEMANTIC_ACTION_SET_SELECTION: info.addAction(AccessibilityNodeInfo.ACTION_SET_SELECTION); break;
                case GpuFrame.SEMANTIC_ACTION_COPY: info.addAction(AccessibilityNodeInfo.ACTION_COPY); break;
                case GpuFrame.SEMANTIC_ACTION_CUT: info.addAction(AccessibilityNodeInfo.ACTION_CUT); break;
                case GpuFrame.SEMANTIC_ACTION_PASTE: info.addAction(AccessibilityNodeInfo.ACTION_PASTE); break;
                case GpuFrame.SEMANTIC_ACTION_SET_PROGRESS:
                    if (Build.VERSION.SDK_INT >= 24) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);
                    break;
                case GpuFrame.SEMANTIC_ACTION_CLEAR_SELECTION: info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_SELECTION); break;
                case GpuFrame.SEMANTIC_ACTION_SELECT_ITEM: {
                    Long index = action.index();
                    info.addAction(new AccessibilityNodeInfo.AccessibilityAction(SELECT_ITEM + (index == null ? 0 : index.intValue()), action.label()));
                    break;
                }
                case GpuFrame.SEMANTIC_ACTION_SHOW_ON_SCREEN: info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN); break;
                default: break;
            }
        }
        info.setAccessibilityFocused(accessibilityFocused);
        info.addAction(accessibilityFocused ? AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS : AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
        Rect rect = pixels(node.bounds(), scale), relative = new Rect(rect);
        if (parentJson != null) {
            GpuFrame.SemanticNode parent = new GpuFrame.SemanticNode(parentJson);
            Rect group = pixels(parent.bounds(), scale);
            relative.offset(-group.left, -group.top);
            info.setParent(host, (int) parent.id());
        } else {
            info.setParent(host);
        }
        long[] children = node.children();
        for (int i = 0; children != null && i < children.length; i++) info.addChild(host, (int) children[i]);
        info.setBoundsInParent(relative);
        int[] location = new int[2]; host.getLocationOnScreen(location); rect.offset(location[0], location[1]);
        info.setBoundsInScreen(rect);
    }

    /** Translate an Android action into the shared action opcode, or -1. */
    static int action(int action) {
        if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) return FOCUS;
        if (action == AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) return CLEAR_FOCUS;
        if (action == AccessibilityNodeInfo.ACTION_CLICK || action == AccessibilityNodeInfo.ACTION_EXPAND
            || action == AccessibilityNodeInfo.ACTION_COLLAPSE) return CLICK;
        if (action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId()) return SHOW_ON_SCREEN;
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD) return SCROLL_FORWARD;
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) return SCROLL_BACKWARD;
        if (action == SET_PROGRESS) return PROGRESS;
        if (action == AccessibilityNodeInfo.ACTION_SET_TEXT) return SET_TEXT;
        if (action == AccessibilityNodeInfo.ACTION_SET_SELECTION) return SET_SELECTION;
        if (action == AccessibilityNodeInfo.ACTION_CLEAR_SELECTION) return CLEAR_SELECTION;
        if (action == AccessibilityNodeInfo.ACTION_COPY) return COPY;
        if (action == AccessibilityNodeInfo.ACTION_CUT) return CUT;
        if (action == AccessibilityNodeInfo.ACTION_PASTE) return PASTE;
        if (action >= SELECT_ITEM && action < SELECT_ITEM + 128) return SELECT;
        return -1;
    }
}
