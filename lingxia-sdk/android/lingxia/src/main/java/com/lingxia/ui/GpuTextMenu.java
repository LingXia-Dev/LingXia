package com.lingxia.ui;

import android.graphics.Rect;
import android.view.ActionMode;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import java.util.function.Supplier;

/** Platform editing chrome; text and selection remain in the GPU editor. */
final class GpuTextMenu {
    private final View view;
    private final GpuIme ime;
    private final Supplier<Rect> bounds;
    private ActionMode mode;
    private GpuIme.Ticket owner;
    private String text;
    private boolean selectedAtShow;
    private int preparedOptions;
    private final Rect anchor=new Rect();

    GpuTextMenu(View view,GpuIme ime,Supplier<Rect> bounds) {
        this.view=view; this.ime=ime; this.bounds=bounds;
    }
    private boolean current() {
        GpuIme.Ticket now=ime.capture();
        return ime.menuActive() && owner != null && owner.epoch == now.epoch && owner.connection == now.connection;
    }
    void show() {
        dismiss();
        if (!ime.menuActive()) return;
        owner=ime.capture(); text=ime.menuText();
        selectedAtShow=ime.menuSelected();
        anchor.set(bounds.get());
        mode=view.startActionMode(new ActionMode.Callback2() {
            @Override public boolean onCreateActionMode(ActionMode actionMode,Menu menu) {
                menu.add(0,android.R.id.cut,0,android.R.string.cut).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0,android.R.id.copy,1,android.R.string.copy).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0,android.R.id.paste,2,android.R.string.paste).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0,android.R.id.selectAll,3,android.R.string.selectAll).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0,android.R.id.undo,4,com.lingxia.lxapp.R.string.lingxia_ui_undo).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0,android.R.id.redo,5,com.lingxia.lxapp.R.string.lingxia_ui_redo).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                prepare(menu); return current();
            }
            @Override public boolean onPrepareActionMode(ActionMode actionMode,Menu menu) { prepare(menu); return true; }
            @Override public boolean onActionItemClicked(ActionMode actionMode,MenuItem item) {
                if (mode != actionMode || !current()) { actionMode.finish(); return false; }
                boolean applied=ime.menuAction(item.getItemId());
                if (applied && item.getItemId() != android.R.id.selectAll) dismiss();
                else update();
                return applied;
            }
            @Override public void onDestroyActionMode(ActionMode actionMode) {
                if (mode == actionMode) { mode=null; owner=null; text=null; }
            }
            @Override public void onGetContentRect(ActionMode actionMode,View target,Rect outRect) { anchor.set(bounds.get()); outRect.set(anchor); }
        },ActionMode.TYPE_FLOATING);
        if (mode == null) { owner=null; text=null; }
    }
    private void prepare(Menu menu) {
        preparedOptions=options();
        boolean export=current() && (preparedOptions & 1) != 0;
        menu.findItem(android.R.id.cut).setVisible(export);
        menu.findItem(android.R.id.copy).setVisible(export);
        // Do not read clipboard contents merely to show the toolbar.
        menu.findItem(android.R.id.paste).setEnabled(current());
        menu.findItem(android.R.id.selectAll).setVisible(current() && (preparedOptions & 2) != 0);
        menu.findItem(android.R.id.undo).setVisible(current() && (preparedOptions & 4) != 0);
        menu.findItem(android.R.id.redo).setVisible(current() && (preparedOptions & 8) != 0);
    }
    private int options() { return (ime.menuSelected() && !ime.menuSecure() ? 1 : 0) | (!ime.menuText().isEmpty() ? 2 : 0)
        | (ime.menuUndo() ? 4 : 0) | (ime.menuRedo() ? 8 : 0); }
    void update() {
        if (mode == null) return;
        if (!current() || !text.equals(ime.menuText()) || (selectedAtShow && !ime.menuSelected())) { dismiss(); return; }
        // Floating toolbar invalidation can relayout the host. Stable frames must stay idle.
        if (preparedOptions != options()) { preparedOptions=options(); mode.invalidate(); }
        Rect next=bounds.get();
        if (!anchor.equals(next)) { anchor.set(next); mode.invalidateContentRect(); }
    }
    void dismiss() {
        ActionMode previous=mode; mode=null; owner=null; text=null;
        if (previous != null) previous.finish();
    }
}
