package com.lingxia.ui;

import android.graphics.Rect;
import android.view.ActionMode;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import org.json.JSONArray;
import org.json.JSONObject;

/** Draws the floating text menu the core decides (`GpuPlatform.textMenu`); nothing here chooses. */
final class GpuTextMenu {
    interface Host {
        /** The core runs a static-text action; false leaves it to the editor. */
        boolean coreAction(int code);
        /** Run an editor action through its input connection. */
        boolean editorAction(int itemId);
        /** The user closed the menu (back, outside touch). */
        void dismissed();
    }
    /** Core action codes, in `MenuAction` order. */
    static final String[] ACTIONS = {"cut", "copy", "paste", "select_all", "undo", "redo"};
    private static final int[] ITEMS = {android.R.id.cut, android.R.id.copy, android.R.id.paste,
        android.R.id.selectAll, android.R.id.undo, android.R.id.redo};
    private final View view;
    private final Host host;
    private ActionMode mode;
    private JSONObject menu;
    private final Rect anchor = new Rect();

    GpuTextMenu(View view, Host host) { this.view = view; this.host = host; }

    /** Show or update `menu` (`{target, anchor, actions}`), or close it for null. */
    void show(JSONObject menu, float scale) {
        if (menu == null) { dismiss(); return; }
        JSONArray rect = menu.optJSONArray("anchor");
        Rect next = rect == null ? new Rect() : new Rect((int)(rect.optDouble(0) * scale), (int)(rect.optDouble(1) * scale),
            (int)((rect.optDouble(0) + rect.optDouble(2)) * scale), (int)((rect.optDouble(1) + rect.optDouble(3)) * scale));
        this.menu = menu;
        if (mode != null) {
            mode.invalidate();
            if (!anchor.equals(next)) { anchor.set(next); mode.invalidateContentRect(); }
            return;
        }
        anchor.set(next);
        mode = view.startActionMode(new ActionMode.Callback2() {
            @Override public boolean onCreateActionMode(ActionMode actionMode, Menu items) { fill(items); return true; }
            @Override public boolean onPrepareActionMode(ActionMode actionMode, Menu items) { fill(items); return true; }
            @Override public boolean onActionItemClicked(ActionMode actionMode, MenuItem item) {
                if (actionMode != mode) return false;
                int code = code(item.getItemId());
                if (code < 0) return false;
                if (host.coreAction(code)) return true;
                return editor() && host.editorAction(item.getItemId());
            }
            @Override public void onDestroyActionMode(ActionMode actionMode) {
                if (actionMode != mode) return;
                mode = null; GpuTextMenu.this.menu = null;
                host.dismissed();
            }
            @Override public void onGetContentRect(ActionMode actionMode, View target, Rect outRect) { outRect.set(anchor); }
        }, ActionMode.TYPE_FLOATING);
    }

    private boolean editor() {
        JSONObject target = menu == null ? null : menu.optJSONObject("target");
        return target != null && "editor".equals(target.optString("kind"));
    }

    private void fill(Menu items) {
        items.clear();
        JSONArray actions = menu == null ? null : menu.optJSONArray("actions");
        for (int i = 0; actions != null && i < actions.length(); i++) {
            int code = java.util.Arrays.asList(ACTIONS).indexOf(actions.optString(i));
            if (code < 0) continue;
            int title = code == 4 ? com.lingxia.lxapp.R.string.lingxia_ui_undo
                : code == 5 ? com.lingxia.lxapp.R.string.lingxia_ui_redo
                : new int[]{android.R.string.cut, android.R.string.copy, android.R.string.paste, android.R.string.selectAll}[code];
            items.add(0, ITEMS[code], i, title).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
        }
    }

    private static int code(int itemId) {
        for (int i = 0; i < ITEMS.length; i++) if (ITEMS[i] == itemId) return i;
        return -1;
    }

    /** Close it without reporting a user dismissal. */
    void dismiss() {
        ActionMode open = mode; mode = null; menu = null;
        if (open != null) open.finish();
    }
}
