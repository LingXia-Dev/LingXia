package com.lingxia.ui;

import android.graphics.*;
import android.graphics.drawable.Drawable;
import android.view.MotionEvent;
import android.view.View;
import org.json.JSONArray;
import org.json.JSONObject;

/** Handles for selectable static text; the selection and its menu live in the core. */
final class GpuStaticSelection extends Drawable {
    interface Host {
        JSONObject frame();
        float scale();
        void move(String identity, int which, float x, float y);
        /** A handle was grabbed or let go; the core hides and restores the text menu. */
        void dragging(boolean active);
    }
    private final View view;
    private final Host host;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private String dragging;
    private int which, pointer;
    private float dx, dy;

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

    /** After each presented frame: handles follow the selection or go with it. */
    void update() {
        if (node() == null) release();
        invalidateSelf();
    }

    private void release() {
        if (dragging == null) return;
        dragging = null;
        host.dragging(false);
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
            host.dragging(true);
            return true;
        }
        if (dragging == null) return false;
        if (action == MotionEvent.ACTION_CANCEL || action == MotionEvent.ACTION_POINTER_DOWN) { release(); update(); return true; }
        int index = event.findPointerIndex(pointer);
        if (index < 0) { release(); update(); return true; }
        if (action == MotionEvent.ACTION_MOVE || action == MotionEvent.ACTION_UP) {
            JSONObject node = node();
            float scale = host.scale();
            float[] drag = node == null ? null : GpuView.nativeHandleDrag(node.toString(), event.getX(index) / scale - dx, event.getY(index) / scale - dy);
            if (drag != null && drag[4] != 0) host.move(dragging, which, drag[0], drag[1]);
            if (action == MotionEvent.ACTION_UP) { release(); update(); }
        }
        return true;
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
