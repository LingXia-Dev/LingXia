package com.lingxia.ui;

import android.os.Handler;
import android.os.Looper;

/** A single phase timer; repeated presentations of unchanged text do not restart it. */
final class GpuCaretBlink {
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Runnable invalidate;
    private boolean running, visible = true;
    private String text;
    private int anchor, focus, compositionStart, compositionEnd;
    private final Runnable tick = new Runnable() {
        @Override public void run() {
            if (!running) return;
            visible = !visible;
            invalidate.run();
            if (running) handler.postDelayed(this, 500);
        }
    };

    GpuCaretBlink(Runnable invalidate) { this.invalidate = invalidate; }
    boolean visible() { return visible; }
    void update(boolean eligible, String text, int anchor, int focus, int start, int end) {
        boolean changed = !java.util.Objects.equals(this.text, text) || this.anchor != anchor
            || this.focus != focus || compositionStart != start || compositionEnd != end;
        this.text = text; this.anchor = anchor; this.focus = focus;
        compositionStart = start; compositionEnd = end;
        boolean next = eligible && anchor == focus;
        if (next == running && !changed) return;
        handler.removeCallbacks(tick);
        running = next;
        boolean repaint = !visible;
        visible = true;
        if (running) handler.postDelayed(tick, 500);
        if (repaint) invalidate.run();
    }
    void stop() {
        handler.removeCallbacks(tick);
        running = false; visible = true; text = null;
        // Focus teardown already schedules its own frame after the triggering action.
    }
}
