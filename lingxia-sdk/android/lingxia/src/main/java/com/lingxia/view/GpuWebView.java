package com.lingxia.view;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Rect;
import android.os.Handler;
import android.os.Looper;
import android.view.Choreographer;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewParent;
import android.webkit.WebView;
import android.widget.FrameLayout;
import com.lingxia.webview.LingXiaWebView;
import java.util.HashMap;
import java.util.HashSet;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Hosts live embedded {@link LingXiaWebView} instances for a {@link GpuView}, capturing each
 * WebView's post-commit frames into its {@link GpuVideo} {@code SurfaceTexture} sink so Vello
 * composites them with rounded corners, clips, overlays and {@code PixelCopy} screenshots.
 */
final class GpuWebView extends FrameLayout {
    public interface Provider {
        View resolve(String webId);
    }

    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final ConcurrentHashMap<String, Slot> LIVE_SLOTS = new ConcurrentHashMap<>();
    private static final AtomicLong NEXT_VISUAL_REQ = new AtomicLong(1);

    /** Capture the web view of [key] again: its sink appeared, or took a capture that had to wait. */
    static void recapture(String key) {
        MAIN.post(() -> {
            Slot slot = LIVE_SLOTS.get(key);
            if (slot != null) {
                slot.invalidateCapture();
            }
        });
    }

    private final GpuView owner;
    private Provider provider;
    private final HashMap<String, Slot> slots = new HashMap<>();
    private Slot touchOwner;
    /** Not yet past the touch slop: a drag the web page cannot scroll goes to the View page. */
    private boolean touchDeciding;
    private float touchDownX;
    private float touchDownY;

    /** What [dispatchPointer] did with an event. */
    static final int PAGE = 0;
    static final int WEB = 1;
    /** The web page could not scroll this drag: replay it to the View page from its down. */
    static final int HANDED_OFF = 2;
    private float touchLeftPx;
    private float touchTopPx;

    GpuWebView(Context context, GpuView owner) {
        super(context);
        this.owner = owner;
        setClipChildren(false);
        setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    void setProvider(Provider provider) {
        this.provider = provider;
    }

    boolean hasFocusedWebView() {
        for (Slot slot : slots.values()) {
            if (slot.hasFocus()) {
                return true;
            }
        }
        return false;
    }

    void clearWebFocus() {
        for (Slot slot : slots.values()) {
            if (slot.hasFocus()) {
                slot.clearFocus();
            }
        }
    }

    void cancelTouch() {
        if (touchOwner == null) {
            return;
        }
        long now = android.os.SystemClock.uptimeMillis();
        MotionEvent cancel = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0f, 0f, 0);
        try {
            touchOwner.dispatchTouchEvent(cancel);
        } finally {
            cancel.recycle();
            touchOwner = null;
        }
    }

    /** The down of a drag just handed off, at the point and time it began. */
    MotionEvent handedOffDown(MotionEvent current) {
        MotionEvent down = MotionEvent.obtain(current);
        down.setAction(MotionEvent.ACTION_DOWN);
        down.setLocation(touchDownX, touchDownY);
        return down;
    }

    int dispatchPointer(MotionEvent event, String hitSpec, Runnable releaseViewIme) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) {
            cancelTouch();
            if (hitSpec == null || hitSpec.isEmpty()) {
                clearWebFocus();
                return PAGE;
            }
            String[] parts = hitSpec.split("\n");
            if (parts.length < 3) {
                return PAGE;
            }
            Slot slot = slots.get(parts[0]);
            if (slot == null || slot.degraded || slot.webview == null) {
                return PAGE;
            }
            try {
                touchLeftPx = Float.parseFloat(parts[1]);
                touchTopPx = Float.parseFloat(parts[2]);
            } catch (NumberFormatException ignored) {
                return PAGE;
            }
            touchDeciding = !slot.handlesDrags;
            touchDownX = event.getX();
            touchDownY = event.getY();
            if (releaseViewIme != null) {
                releaseViewIme.run();
            }
            touchOwner = slot;
            if (getParent() != null) {
                getParent().requestDisallowInterceptTouchEvent(true);
            }
        } else if (touchOwner == null) {
            return PAGE;
        } else if (touchDeciding && action == MotionEvent.ACTION_MOVE) {
            float dx = event.getX() - touchDownX;
            float dy = event.getY() - touchDownY;
            int slop = android.view.ViewConfiguration.get(getContext()).getScaledTouchSlop();
            if (dx * dx + dy * dy > (float) slop * slop) {
                touchDeciding = false;
                View web = touchOwner.webview;
                // A finger moving down scrolls the content toward its top.
                boolean scrolls = Math.abs(dy) >= Math.abs(dx)
                    ? web.canScrollVertically(dy > 0 ? -1 : 1)
                    : web.canScrollHorizontally(dx > 0 ? -1 : 1);
                if (!scrolls) {
                    cancelTouch();
                    if (getParent() != null) {
                        getParent().requestDisallowInterceptTouchEvent(false);
                    }
                    return HANDED_OFF;
                }
            }
        }

        Slot target = touchOwner;
        MotionEvent local = MotionEvent.obtain(event);
        local.offsetLocation(-touchLeftPx, -touchTopPx);
        boolean handled;
        try {
            handled = target.dispatchTouchEvent(local);
        } finally {
            local.recycle();
        }
        target.burstCapture(20);
        if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            touchOwner = null;
            touchDeciding = false;
            if (getParent() != null) {
                getParent().requestDisallowInterceptTouchEvent(false);
            }
        }
        return WEB;
    }

    void clearAll() {
        cancelTouch();
        for (Slot slot : slots.values()) {
            slot.detach();
            removeView(slot);
        }
        slots.clear();
    }

    void sync(String mountsJson, float scale) {
        if (mountsJson == null || mountsJson.isEmpty()) {
            clearAll();
            return;
        }
        float density = scale > 0f ? scale : getResources().getDisplayMetrics().density;
        try {
            JSONArray array = new JSONArray(mountsJson);
            if (array.length() == 0) {
                clearAll();
                return;
            }
            HashSet<String> keep = new HashSet<>();
            for (int i = 0; i < array.length(); i++) {
                JSONObject item = array.getJSONObject(i);
                String webId = item.getString("webId");
                String key = item.getString("key");
                keep.add(webId);

                Slot slot = slots.get(webId);
                if (slot == null) {
                    slot = new Slot(getContext(), webId, key);
                    slots.put(webId, slot);
                    addView(slot);
                } else if (!key.equals(slot.key)) {
                    LIVE_SLOTS.remove(slot.key, slot);
                    slot.key = key;
                    LIVE_SLOTS.put(key, slot);
                }

                if (slot.webview == null && provider != null) {
                    View resolved = provider.resolve(webId);
                    if (resolved != null) {
                        slot.attachWebView(resolved);
                    }
                }

                JSONArray bounds = item.getJSONArray("bounds");
                int left = Math.round((float) bounds.getDouble(0) * density);
                int top = Math.round((float) bounds.getDouble(1) * density);
                int width = Math.max(1, Math.round((float) bounds.getDouble(2) * density));
                int height = Math.max(1, Math.round((float) bounds.getDouble(3) * density));
                boolean degraded = item.optBoolean("degraded", false);
                boolean a11yHidden = item.optBoolean("accessibilityHidden", false);
                boolean ready = item.optBoolean("ready", false);
                String src = item.optString("src", "");
                slot.handlesDrags = item.optBoolean("handlesDrags", false);

                slot.update(left, top, width, height, degraded, a11yHidden, ready, src);
            }

            java.util.Iterator<java.util.Map.Entry<String, Slot>> it = slots.entrySet().iterator();
            while (it.hasNext()) {
                java.util.Map.Entry<String, Slot> entry = it.next();
                if (!keep.contains(entry.getKey())) {
                    Slot removed = entry.getValue();
                    if (touchOwner == removed) {
                        cancelTouch();
                    }
                    removed.detach();
                    removeView(removed);
                    it.remove();
                }
            }
        } catch (org.json.JSONException ignored) {
        }
    }

    @Override
    protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        for (Slot slot : slots.values()) {
            slot.layout(slot.slotLeft, slot.slotTop, slot.slotLeft + slot.slotWidth, slot.slotTop + slot.slotHeight);
        }
    }

    @Override
    protected void onMeasure(int widthSpec, int heightSpec) {
        super.onMeasure(widthSpec, heightSpec);
        for (Slot slot : slots.values()) {
            slot.measure(
                MeasureSpec.makeMeasureSpec(slot.slotWidth, MeasureSpec.EXACTLY),
                MeasureSpec.makeMeasureSpec(slot.slotHeight, MeasureSpec.EXACTLY)
            );
        }
    }

    private final class Slot extends FrameLayout implements LingXiaWebView.CustomViewListener {
        final String webId;
        String key;
        View webview;
        View customView;
        int slotLeft;
        int slotTop;
        int slotWidth = 1;
        int slotHeight = 1;
        boolean degraded;
        boolean ready;
        boolean handlesDrags;
        String lastSrc = "";
        private int burstRemaining;
        private boolean capturing;
        private boolean captureScheduled;
        /** Changed since the last capture. The traversal and the frame callback both capture; one per change. */
        private boolean captureDirty = true;
        private boolean detached;
        private final Choreographer.FrameCallback captureCallback = frameTimeNanos -> {
            captureScheduled = false;
            if (!detached) {
                captureToSink();
                if (burstRemaining > 0) {
                    burstRemaining--;
                    invalidateCapture();
                }
            }
        };

        Slot(Context context, String webId, String key) {
            super(context);
            this.webId = webId;
            this.key = key;
            setWillNotDraw(false);
            LIVE_SLOTS.put(key, this);
        }

        void attachWebView(View view) {
            if (view.getParent() instanceof ViewGroup) {
                ((ViewGroup) view.getParent()).removeView(view);
            }
            this.webview = view;
            addView(view, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            if (view instanceof LingXiaWebView) {
                ((LingXiaWebView) view).setCustomViewListener(this);
            }
            if (slotWidth > 0 && slotHeight > 0) {
                measure(
                    MeasureSpec.makeMeasureSpec(slotWidth, MeasureSpec.EXACTLY),
                    MeasureSpec.makeMeasureSpec(slotHeight, MeasureSpec.EXACTLY)
                );
                layout(slotLeft, slotTop, slotLeft + slotWidth, slotTop + slotHeight);
            }
            scheduleVisualStateCapture();
            invalidateCapture();
        }

        void update(int left, int top, int width, int height, boolean degraded, boolean a11yHidden, boolean ready, String src) {
            boolean sizeChanged = this.slotWidth != width || this.slotHeight != height
                || this.slotLeft != left || this.slotTop != top;
            boolean becameReady = !this.ready && ready;
            boolean stateChanged = this.degraded != degraded || this.ready != ready;
            boolean srcChanged = src != null && !src.equals(this.lastSrc);
            this.slotLeft = left;
            this.slotTop = top;
            this.slotWidth = width;
            this.slotHeight = height;
            this.degraded = degraded;
            this.ready = ready;
            this.lastSrc = src != null ? src : "";
            int importance = (degraded || a11yHidden)
                ? View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
                : View.IMPORTANT_FOR_ACCESSIBILITY_AUTO;
            if (getImportantForAccessibility() != importance) {
                setImportantForAccessibility(importance);
            }
            if (sizeChanged) {
                measure(
                    MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
                    MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY)
                );
                layout(left, top, left + width, top + height);
            }
            if (becameReady || (sizeChanged && ready) || (srcChanged && ready)) {
                scheduleVisualStateCapture();
                burstCapture(24);
            } else if (sizeChanged || stateChanged) {
                invalidateCapture();
            }
        }

        void burstCapture(int frames) {
            if (detached) {
                return;
            }
            if (frames > burstRemaining) {
                burstRemaining = frames;
            }
            invalidateCapture();
        }

        @Override
        public void onShowCustomView(View view) {
            if (customView != null) {
                removeView(customView);
            }
            customView = view;
            if (view.getParent() instanceof ViewGroup) {
                ((ViewGroup) view.getParent()).removeView(view);
            }
            addView(view, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            invalidateCapture();
        }

        @Override
        public void onHideCustomView() {
            if (customView != null) {
                removeView(customView);
                customView = null;
                invalidateCapture();
            }
        }

        void detach() {
            detached = true;
            if (captureScheduled) {
                captureScheduled = false;
                Choreographer.getInstance().removeFrameCallback(captureCallback);
            }
            LIVE_SLOTS.remove(key, this);
            if (webview instanceof LingXiaWebView) {
                ((LingXiaWebView) webview).setCustomViewListener(null);
            }
            if (customView != null) {
                removeView(customView);
                customView = null;
            }
            if (webview != null) {
                removeView(webview);
                webview = null;
            }
        }

        private void scheduleVisualStateCapture() {
            if (!(webview instanceof WebView)) {
                return;
            }
            WebView wv = (WebView) webview;
            try {
                wv.postVisualStateCallback(NEXT_VISUAL_REQ.getAndIncrement(), new WebView.VisualStateCallback() {
                    @Override
                    public void onComplete(long requestId) {
                        if (!detached) {
                            invalidateCapture();
                        }
                    }
                });
            } catch (Throwable ignored) {
            }
        }

        void invalidateCapture() {
            if (detached) {
                return;
            }
            captureDirty = true;
            invalidate();
            if (!captureScheduled) {
                captureScheduled = true;
                Choreographer.getInstance().postFrameCallback(captureCallback);
            }
        }

        private boolean hasCommittedContent() {
            if (ready || customView != null) {
                return true;
            }
            if (webview instanceof WebView) {
                WebView wv = (WebView) webview;
                String url = wv.getUrl();
                return url != null && !url.isEmpty() && !"about:blank".equals(url) && wv.getContentHeight() > 0;
            }
            return webview != null;
        }

        @Override
        public void draw(Canvas canvas) {
            if (capturing) {
                super.draw(canvas);
                return;
            }
            captureToSink();
        }

        @Override
        protected void dispatchDraw(Canvas canvas) {
            if (capturing) {
                super.dispatchDraw(canvas);
                return;
            }
            captureToSink();
        }

        @Override
        protected void onSizeChanged(int w, int h, int oldw, int oldh) {
            super.onSizeChanged(w, h, oldw, oldh);
            captureDirty = true;
        }

        private void captureToSink() {
            if (!captureDirty || degraded || webview == null || !hasCommittedContent()) {
                return;
            }
            int w = getWidth();
            int h = getHeight();
            if (w <= 0 || h <= 0) {
                return;
            }
            GpuVideo sink = GpuVideo.webSink(key);
            if (sink == null) {
                return;
            }
            capturing = true;
            boolean drawn;
            try {
                drawn = sink.drawWebView(this, w, h);
            } finally {
                capturing = false;
            }
            if (drawn) {
                captureDirty = false;
                owner.onWebViewTextureUpdated();
            }
        }

        @Override
        public void onDescendantInvalidated(View child, View target) {
            super.onDescendantInvalidated(child, target);
            if (!capturing) {
                invalidateCapture();
            }
        }

        @Override
        @SuppressWarnings("deprecation")
        public ViewParent invalidateChildInParent(int[] location, Rect dirty) {
            if (!capturing) {
                invalidateCapture();
            }
            return super.invalidateChildInParent(location, dirty);
        }
    }
}
