package com.lingxia.ui;

import android.app.AlertDialog;
import android.content.Context;
import android.graphics.Rect;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ArrayAdapter;
import android.widget.ListPopupWindow;
import android.widget.TextView;
import java.util.function.Consumer;
import org.json.JSONArray;
import org.json.JSONObject;

/** The platform single-choice picker for a `<select>`; the choice goes back through `{op:"select"}`. */
final class GpuPicker {
    interface Host {
        /** Send an input-protocol request; the reply arrives on the main thread. */
        void request(JSONObject request, Consumer<JSONObject> reply);
        void render(boolean rebuild);
        /** The picker for `identity` opened (or closed, null), for its expanded state. */
        void open(String identity);
    }
    /** Up to this many options drop down from the box; more open a scrolling dialog. */
    static final int DROPDOWN_OPTIONS = 8;
    private final View anchor;
    private final Host host;
    private ListPopupWindow popup;
    private AlertDialog dialog;

    GpuPicker(View anchor, Host host) { this.anchor = anchor; this.host = host; }

    /** Open for `node` (a frame node with `select`), drawn for frame `generation` at `scale`. */
    void show(JSONObject node, long generation, Rect bounds) {
        dismiss();
        JSONObject select = node.optJSONObject("select");
        JSONArray options = select == null ? null : select.optJSONArray("options");
        if (options == null || options.length() == 0) return;
        String identity = node.optString("identity");
        String[] labels = new String[options.length()];
        boolean[] enabled = new boolean[options.length()];
        for (int i = 0; i < labels.length; i++) {
            JSONObject option = options.optJSONObject(i);
            labels[i] = option == null ? "" : option.optString("label");
            enabled[i] = option != null && option.optBoolean("enabled", true);
        }
        int selected = select.isNull("selected") ? -1 : select.optInt("selected", -1);
        Context context = anchor.isAttachedToWindow() ? anchor.getRootView().getContext() : anchor.getContext();
        ArrayAdapter<String> adapter = new ArrayAdapter<String>(context, android.R.layout.simple_list_item_single_choice, labels) {
            @Override public boolean isEnabled(int position) { return enabled[position]; }
            @Override public boolean areAllItemsEnabled() { return false; }
            @Override public View getView(int position, View convert, ViewGroup parent) {
                View row = super.getView(position, convert, parent);
                row.setEnabled(enabled[position]);
                if (row instanceof TextView) ((TextView)row).setAlpha(enabled[position] ? 1f : 0.38f);
                return row;
            }
        };
        Consumer<Integer> choose = index -> {
            dismiss();
            if (index < 0 || index >= labels.length || !enabled[index]) return;
            JSONObject request;
            try { request = new JSONObject().put("op", "select").put("identity", identity).put("generation", generation).put("index", index); }
            catch (org.json.JSONException impossible) { throw new IllegalArgumentException(impossible); }
            host.request(request, reply -> host.render(reply != null && reply.optBoolean("needs_view_render")));
        };
        host.open(identity);
        if (labels.length <= DROPDOWN_OPTIONS) {
            ListPopupWindow list = new ListPopupWindow(context);
            list.setAdapter(adapter);
            list.setAnchorView(anchor);
            list.setHorizontalOffset(bounds.left);
            list.setVerticalOffset(bounds.bottom - anchor.getHeight());
            list.setWidth(Math.max(bounds.width(), 1));
            list.setModal(true);
            list.setOnItemClickListener((parent, view, position, id) -> choose.accept(position));
            list.setOnDismissListener(() -> { if (popup == list) { popup = null; host.open(null); } });
            popup = list;
            list.show();
            if (list.getListView() != null) {
                list.getListView().setChoiceMode(android.widget.ListView.CHOICE_MODE_SINGLE);
                if (selected >= 0) list.getListView().setItemChecked(selected, true);
            }
        } else {
            AlertDialog shown = new AlertDialog.Builder(context)
                .setSingleChoiceItems(adapter, selected, (which, index) -> choose.accept(index))
                .create();
            shown.setOnDismissListener(ignored -> { if (dialog == shown) { dialog = null; host.open(null); } });
            dialog = shown;
            shown.show();
        }
    }

    boolean showing() { return popup != null || dialog != null; }
    ListPopupWindow dropdown() { return popup; }
    AlertDialog list() { return dialog; }

    /** Close without a choice; nothing is sent. */
    void dismiss() {
        ListPopupWindow list = popup; AlertDialog shown = dialog;
        popup = null; dialog = null;
        if (list != null) list.dismiss();
        if (shown != null) shown.dismiss();
        if (list != null || shown != null) host.open(null);
    }
}
