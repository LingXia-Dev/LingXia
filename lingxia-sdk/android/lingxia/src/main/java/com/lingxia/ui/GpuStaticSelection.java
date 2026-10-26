package com.lingxia.ui;

import android.graphics.*;
import android.graphics.drawable.Drawable;
import android.view.ActionMode;
import android.view.Menu;
import android.view.MenuItem;
import android.view.MotionEvent;
import android.view.View;
import org.json.JSONArray;
import org.json.JSONObject;

/** Handles and Copy/Select all for selectable static text; the selection itself lives in the core. */
final class GpuStaticSelection extends Drawable {
    interface Host {
        JSONObject frame();
        float scale();
        void move(String identity, int which, float x, float y);
        void select(String identity, int start, int end);
        void copy();
        void clear();
    }
    private final View view;
    private final Host host;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private ActionMode mode;
    private String dragging;
    private int which, pointer;
    private float dx, dy;
    private final Rect anchor = new Rect();

    GpuStaticSelection(View view, Host host) { this.view = view; this.host = host; paint.setColor(0xff465cda); }

    /** The node carrying a static-text selection in the presented frame, or null. */
    JSONObject node() {
        JSONObject frame = host.frame();
        JSONArray nodes = frame == null ? null : frame.optJSONArray("nodes");
        for (int i = 0; nodes != null && i < nodes.length(); i++) {
            JSONObject node = nodes.optJSONObject(i);
            if (node != null && node.optJSONObject("input") == null && node.optJSONObject("text_selection") != null) return node;
        }
        return null;
    }
    boolean dragging() { return dragging != null; }

    /** After each presented frame: chrome follows the selection or goes with it. */
    void update() {
        JSONObject node = node();
        if (node == null) { dragging = null; dismiss(); }
        else if (dragging == null) show(node);
        invalidateSelf();
    }

    boolean touch(MotionEvent event) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) {
            dragging = null;
            JSONObject node = node();
            if (node == null) return false;
            float scale = host.scale(), x = event.getX() / scale, y = event.getY() / scale;
            float[] hit = GpuView.nativeHandleHit(node.toString(), x, y, false);
            if (hit == null) return false;
            dragging = node.optString("identity"); which = (int)hit[0]; pointer = event.getPointerId(0);
            dx = x - hit[1]; dy = y - hit[3];
            dismiss();
            return true;
        }
        if (dragging == null) return false;
        if (action == MotionEvent.ACTION_CANCEL || action == MotionEvent.ACTION_POINTER_DOWN) { dragging = null; update(); return true; }
        int index = event.findPointerIndex(pointer);
        if (index < 0) { dragging = null; update(); return true; }
        if (action == MotionEvent.ACTION_MOVE || action == MotionEvent.ACTION_UP) {
            JSONObject node = node();
            float scale = host.scale();
            float[] drag = node == null ? null : GpuView.nativeHandleDrag(node.toString(), event.getX(index) / scale - dx, event.getY(index) / scale - dy);
            if (drag != null && drag[4] != 0) host.move(dragging, which, drag[0], drag[1]);
            if (action == MotionEvent.ACTION_UP) { dragging = null; update(); }
        }
        return true;
    }

    private Rect bounds(JSONObject node) {
        JSONArray rects = node.optJSONObject("text_selection").optJSONArray("selection");
        float scale = host.scale();
        RectF union = null;
        for (int i = 0; rects != null && i < rects.length(); i++) {
            JSONArray r = rects.optJSONArray(i);
            RectF rect = new RectF((float)r.optDouble(0), (float)r.optDouble(1), (float)(r.optDouble(0) + r.optDouble(2)), (float)(r.optDouble(1) + r.optDouble(3)));
            if (union == null) union = rect; else union.union(rect);
        }
        if (union == null) return GpuAccessibility.pixels(node.optJSONArray("bounds"), scale);
        return new Rect((int)(union.left * scale), (int)(union.top * scale), (int)(union.right * scale), (int)(union.bottom * scale));
    }

    private void show(JSONObject node) {
        anchor.set(bounds(node));
        if (mode != null) { mode.invalidateContentRect(); return; }
        final String identity = node.optString("identity");
        final int length = node.optString("text").length();
        mode = view.startActionMode(new ActionMode.Callback2() {
            @Override public boolean onCreateActionMode(ActionMode actionMode, Menu menu) {
                menu.add(0, android.R.id.copy, 0, android.R.string.copy).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                menu.add(0, android.R.id.selectAll, 1, android.R.string.selectAll).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM);
                return true;
            }
            @Override public boolean onPrepareActionMode(ActionMode actionMode, Menu menu) { return true; }
            @Override public boolean onActionItemClicked(ActionMode actionMode, MenuItem item) {
                if (item.getItemId() == android.R.id.copy) { host.copy(); return true; }
                if (item.getItemId() == android.R.id.selectAll) { host.select(identity, 0, length); return true; }
                return false;
            }
            @Override public void onDestroyActionMode(ActionMode actionMode) {
                if (mode != actionMode) return;
                mode = null;
                // Dismissed by the user (back, outside touch): the selection goes too.
                if (dragging == null && node() != null) host.clear();
            }
            @Override public void onGetContentRect(ActionMode actionMode, View target, Rect outRect) { outRect.set(anchor); }
        }, ActionMode.TYPE_FLOATING);
    }

    void dismiss() {
        if (mode == null) return;
        ActionMode open = mode; mode = null; open.finish();
    }

    @Override public void draw(Canvas canvas) {
        JSONObject node = node(); if (node == null) return;
        float factor = host.scale(); int saved = canvas.save(); canvas.scale(factor, factor);
        for (int i = 0; i < 2; i++) {
            float[] end = GpuView.nativeHandleEndpoint(node.toString(), i);
            if (end == null) continue;
            canvas.drawRect(end[0] - 1, end[1] - 1, end[0] + 1, end[1] + 7, paint);
            canvas.drawCircle(end[0], end[1] + 8, 7, paint);
        }
        canvas.restoreToCount(saved);
    }
    @Override public void setAlpha(int alpha) { paint.setAlpha(alpha); invalidateSelf(); }
    @Override public void setColorFilter(ColorFilter filter) { paint.setColorFilter(filter); invalidateSelf(); }
    @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
}
