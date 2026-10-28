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
import com.lingxia.ui.generated.GpuFrame;
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
        GpuFrame.Node frameNode = new GpuFrame.Node(node);
        GpuFrame.Select select = frameNode.select();
        GpuFrame.SelectOption[] options = select == null ? null : select.options();
        if (options == null || options.length == 0) return;
        String identity = frameNode.identity();
        String[] labels = new String[options.length];
        boolean[] enabled = new boolean[options.length];
        for (int i = 0; i < labels.length; i++) {
            labels[i] = options[i] == null || options[i].label() == null ? "" : options[i].label();
            enabled[i] = options[i] != null && options[i].enabled();
        }
        Long chosen = select.selected();
        int selected = chosen == null ? -1 : chosen.intValue();
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
            JSONObject request = GpuFrame.InputRequest.select(identity, generation, index);
            host.request(request, reply -> host.render(reply != null && Boolean.TRUE.equals(new GpuFrame.InputReply(reply).needsViewRender())));
        };
        host.open(identity);
        if (labels.length <= DROPDOWN_OPTIONS) {
            ListPopupWindow list = new ListPopupWindow(context);
            list.setAdapter(adapter);
            list.setAnchorView(anchor);
            list.setHorizontalOffset(bounds.left);
            int width = Math.max(bounds.width(), 1);
            Rect visible = new Rect(); int[] at = new int[2];
            anchor.getWindowVisibleDisplayFrame(visible); anchor.getLocationOnScreen(at);
            int[] place = place(bounds.top, bounds.bottom, anchor.getHeight(), at[1], visible.top, visible.bottom,
                contentHeight(list, adapter, width, context));
            list.setVerticalOffset(place[0]);
            list.setHeight(place[1]);
            list.setWidth(width);
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

    /**
     * `{verticalOffset, height}` for a dropdown under the anchor's bottom edge: below the box when
     * the options fit there or it has the most room, else above it; clipped to that room.
     */
    static int[] place(int boxTop, int boxBottom, int anchorHeight, int anchorScreenTop,
                       int visibleTop, int visibleBottom, int content) {
        int below = Math.max(0, visibleBottom - (anchorScreenTop + boxBottom));
        int above = Math.max(0, anchorScreenTop + boxTop - visibleTop);
        if (content <= below || below >= above) return new int[]{boxBottom - anchorHeight, Math.min(content, below)};
        int height = Math.min(content, above);
        return new int[]{boxTop - height - anchorHeight, height};
    }

    /** Every option row plus the popup background's padding. */
    private static int contentHeight(ListPopupWindow list, ArrayAdapter<String> adapter, int width, Context context) {
        Rect padding = new Rect();
        if (list.getBackground() != null) list.getBackground().getPadding(padding);
        int height = padding.top + padding.bottom;
        android.widget.FrameLayout parent = new android.widget.FrameLayout(context);
        int widthSpec = View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY);
        int heightSpec = View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED);
        for (int i = 0; i < adapter.getCount(); i++) {
            View row = adapter.getView(i, null, parent);
            row.measure(widthSpec, heightSpec);
            height += row.getMeasuredHeight();
        }
        return height;
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
