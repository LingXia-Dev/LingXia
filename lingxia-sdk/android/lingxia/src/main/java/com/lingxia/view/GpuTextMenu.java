package com.lingxia.view;

import android.graphics.Rect;
import android.view.ActionMode;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import com.lingxia.view.generated.GpuFrame;
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
    static final String[] ACTIONS = {GpuFrame.MENU_ACTION_CUT, GpuFrame.MENU_ACTION_COPY, GpuFrame.MENU_ACTION_PASTE,
        GpuFrame.MENU_ACTION_SELECT_ALL, GpuFrame.MENU_ACTION_UNDO, GpuFrame.MENU_ACTION_REDO};
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
        float[] rect = new GpuFrame.TextMenu(menu).anchor();
        Rect next = rect == null || rect.length < 4 ? new Rect() : new Rect((int)(rect[0] * scale), (int)(rect[1] * scale),
            (int)((rect[0] + rect[2]) * scale), (int)((rect[1] + rect[3]) * scale));
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
                // The core closes an editor menu before declining, so read the target first.
                boolean editor = editor();
                if (host.coreAction(code)) return true;
                return editor && host.editorAction(item.getItemId());
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
        GpuFrame.TextMenuTarget target = menu == null ? null : new GpuFrame.TextMenu(menu).target();
        return target != null && GpuFrame.TEXT_MENU_TARGET_EDITOR.equals(target.kind());
    }

    private void fill(Menu items) {
        items.clear();
        String[] actions = menu == null ? null : new GpuFrame.TextMenu(menu).actions();
        for (int i = 0; actions != null && i < actions.length; i++) {
            int code = java.util.Arrays.asList(ACTIONS).indexOf(actions[i]);
            if (code < 0) continue;
            int title = code == 4 ? com.lingxia.lxapp.R.string.lingxia_view_undo
                : code == 5 ? com.lingxia.lxapp.R.string.lingxia_view_redo
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
