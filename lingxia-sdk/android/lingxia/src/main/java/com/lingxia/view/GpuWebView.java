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
                // The page lays out at the box's own size: a scale in motion (a sheet opening)
                // shrinks only `bounds`, and the texture it captures is drawn under that scale.
                JSONArray size = item.optJSONArray("size");
                int left = Math.round((float) bounds.getDouble(0) * density);
                int top = Math.round((float) bounds.getDouble(1) * density);
                int width = Math.max(1, Math.round((float) (size != null ? size.getDouble(0) : bounds.getDouble(2)) * density));
                int height = Math.max(1, Math.round((float) (size != null ? size.getDouble(1) : bounds.getDouble(3)) * density));
                boolean degraded = item.optBoolean("degraded", false);
                boolean a11yHidden = item.optBoolean("accessibilityHidden", false);
                boolean ready = item.optBoolean("ready", false);
                String src = item.optString("src", "");
                slot.handlesDrags = item.optBoolean("handlesDrags", false);

                slot.update(left, top, width, height, degraded, a11yHidden, ready, src);
                if (item.has("dark")) {
                    slot.applyDark(item.optBoolean("dark", false));
                }
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

    private static boolean turns(int fromWidth, int fromHeight, int toWidth, int toHeight) {
        return (fromWidth > fromHeight && toWidth < toHeight) || (fromWidth < fromHeight && toWidth > toHeight);
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
        /** The appearance last handed to {@link #appliedTo}. */
        private boolean appliedDark;
        private View appliedTo;
        private int burstRemaining;
        private boolean capturing;
        private boolean captureScheduled;
        /** Changed since the last capture. The traversal and the frame callback both capture; one per change. */
        private boolean captureDirty = true;
        private boolean detached;
        private int capturedWidth, capturedHeight;
        /** Resized over a good picture: capture again only once the page has drawn at the new size. */
        private boolean awaitingDraw;
        private long awaitingSince;
        /**
         * The first visual-state request made after the page saw the new size. One made earlier
         * is answered by a frame of the old layout, as the resize reaches the page later.
         */
        private long awaitedRequest = Long.MAX_VALUE;
        /** Bumped per resize so an older size probe stops. */
        private int sizeProbe;
        /** The page size the running probe last logged. */
        private String lastProbe;
        private final Runnable drawTimeout = () -> {
            if (awaitingDraw) {
                awaitingDraw = false;
                android.util.Log.i("LingXia.GpuWebView", "resize " + key + " timed out waiting for the page to draw");
                invalidateCapture();
            }
        };
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
                View view = webview;
                if (view instanceof WebView && turnsAgainstScreen(view.getWidth(), view.getHeight(), width, height)) {
                    // Chromium reads a web view turning against the screen's orientation as a
                    // rotation and holds the page at its old size for 200 ms; one axis at a time
                    // is a plain resize.
                    place(left, top, view.getWidth(), height);
                }
                place(left, top, width, height);
            }
            if (sizeChanged && ready && capturedWidth > 0 && webview instanceof WebView && customView == null
                && (width != capturedWidth || height != capturedHeight)) {
                // Until the page redraws at the new size a capture is its old frame cropped, or
                // black: the box keeps the old picture (shrunk into it, or at its own size) until
                // the page has drawn.
                awaitingDraw = true;
                awaitingSince = android.os.SystemClock.uptimeMillis();
                lastProbe = null;
                android.util.Log.i("LingXia.GpuWebView", "resize " + key + " box " + width + "x" + height
                    + " over capture " + capturedWidth + "x" + capturedHeight + " web view "
                    + webview.getWidth() + "x" + webview.getHeight() + " shown " + webview.isShown());
                awaitedRequest = Long.MAX_VALUE;
                removeCallbacks(drawTimeout);
                postDelayed(drawTimeout, 300);
                probePageSize(++sizeProbe);
            }
            if (becameReady || (sizeChanged && ready) || (srcChanged && ready)) {
                scheduleVisualStateCapture();
                burstCapture(24);
            } else if (sizeChanged || stateChanged) {
                invalidateCapture();
            }
        }

        private void place(int left, int top, int width, int height) {
            measure(
                MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
                MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY)
            );
            layout(left, top, left + width, top + height);
        }

        private boolean turnsAgainstScreen(int oldWidth, int oldHeight, int width, int height) {
            if (oldWidth <= 0 || oldHeight <= 0 || oldWidth == width || oldHeight == height) {
                return false;
            }
            android.util.DisplayMetrics screen = getResources().getDisplayMetrics();
            return turns(oldWidth, oldHeight, width, height)
                && turns(width, height, screen.widthPixels, screen.heightPixels);
        }

        /**
         * The page's appearance, live: a WebView reads prefers-color-scheme from its creation
         * context and its theme, and nothing re-dispatches a configuration to it while it lives.
         */
        void applyDark(boolean dark) {
            View view = webview;
            if (view == null || (view == appliedTo && dark == appliedDark)) {
                return;
            }
            appliedTo = view;
            appliedDark = dark;
            android.content.res.Configuration config =
                new android.content.res.Configuration(view.getResources().getConfiguration());
            int night = dark
                ? android.content.res.Configuration.UI_MODE_NIGHT_YES
                : android.content.res.Configuration.UI_MODE_NIGHT_NO;
            config.uiMode = (config.uiMode & ~android.content.res.Configuration.UI_MODE_NIGHT_MASK) | night;
            if (view.getContext() instanceof android.view.ContextThemeWrapper) {
                ((android.view.ContextThemeWrapper) view.getContext()).setTheme(dark
                    ? androidx.appcompat.R.style.Theme_AppCompat
                    : androidx.appcompat.R.style.Theme_AppCompat_Light);
            }
            view.dispatchConfigurationChanged(config);
            if (ready) {
                burstCapture(24);
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
            awaitingDraw = false;
            removeCallbacks(drawTimeout);
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

        /**
         * Asks the page for its viewport in device pixels until it matches the box, then requests
         * the frame that carries that layout. Without visualViewport the timeout releases the box.
         */
        private void probePageSize(int probe) {
            if (!(webview instanceof WebView)) {
                return;
            }
            ((WebView) webview).evaluateJavascript(
                "(function(){var v=window.visualViewport;if(!v)return '';var r=devicePixelRatio*v.scale;"
                    + "return Math.round(v.width*r)+'x'+Math.round(v.height*r)})()",
                result -> {
                    if (detached || !awaitingDraw || probe != sizeProbe || result == null) {
                        return;
                    }
                    String[] size = result.replace("\"", "").split("x");
                    if (size.length != 2) {
                        return;
                    }
                    int pageWidth, pageHeight;
                    try {
                        pageWidth = Integer.parseInt(size[0]);
                        pageHeight = Integer.parseInt(size[1]);
                    } catch (NumberFormatException e) {
                        return;
                    }
                    if (Math.abs(pageWidth - slotWidth) <= 2 && Math.abs(pageHeight - slotHeight) <= 2) {
                        android.util.Log.i("LingXia.GpuWebView", "resize " + key + " page at " + result
                            + " after " + (android.os.SystemClock.uptimeMillis() - awaitingSince) + " ms");
                        awaitedRequest = NEXT_VISUAL_REQ.get();
                        scheduleVisualStateCapture();
                    } else {
                        if (!result.equals(lastProbe)) {
                            // Says whether the view or the page is late: the web view's own size
                            // against what the page reports.
                            lastProbe = result;
                            android.util.Log.i("LingXia.GpuWebView", "resize " + key + " page still at " + result
                                + " after " + (android.os.SystemClock.uptimeMillis() - awaitingSince) + " ms, web view "
                                + webview.getWidth() + "x" + webview.getHeight());
                        }
                        postOnAnimation(() -> probePageSize(probe));
                    }
                });
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
                        if (!detached && (!awaitingDraw || requestId >= awaitedRequest)) {
                            if (awaitingDraw) {
                                android.util.Log.i("LingXia.GpuWebView", "resize " + key + " drawn after "
                                    + (android.os.SystemClock.uptimeMillis() - awaitingSince) + " ms");
                            }
                            awaitingDraw = false;
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

        /** Captures in motion too: the box draws the texture under its transform, so it stays current. */
        private void captureToSink() {
            if (!captureDirty || awaitingDraw || webview == null || !hasCommittedContent()) {
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
            if (capturedWidth > 0 && webview instanceof WebView && customView == null && sink.resizeWebSink(w, h)) {
                // The first draw into resized buffers can come out of the old frame, part black:
                // the box keeps the old picture until the page answers after the resize.
                awaitingDraw = true;
                awaitedRequest = NEXT_VISUAL_REQ.get();
                removeCallbacks(drawTimeout);
                postDelayed(drawTimeout, 300);
                scheduleVisualStateCapture();
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
                if (w != capturedWidth || h != capturedHeight) {
                    // Device runs read a sheet's resize as the box size against the capture it shows.
                    android.util.Log.i("LingXia.GpuWebView", "capture " + key + " " + capturedWidth + "x"
                        + capturedHeight + " -> " + w + "x" + h + " box " + slotWidth + "x" + slotHeight);
                    capturedWidth = w;
                    capturedHeight = h;
                }
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
