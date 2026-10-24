package com.lingxia.ui;

import android.content.Context;
import android.widget.FrameLayout;
import android.os.Bundle;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Rect;
import android.view.Choreographer;
import android.view.MotionEvent;
import android.view.KeyEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import android.widget.TextView;
import android.util.Log;
import org.json.JSONArray;
import org.json.JSONObject;
import java.util.concurrent.ExecutorService;


/** GPU page content. Construct and control on the UI thread; share one executor per app session. */
public final class GpuView extends FrameLayout implements SurfaceHolder.Callback2 {
    /** Standalone hosts call this after loading the library; the SDK initializes it automatically. */
    public static native void initializeNative();

    private static native long nativeLoad(String source);
    static native long nativeImageQueueCreate();
    static native void nativeImageQueueUpdate(long queue, String sources);
    static native String nativeImageQueueStart(long queue);
    static native boolean nativeImageQueueCheck(long queue, long token, boolean complete);
    static native void nativeImageQueueReset(long queue, boolean close);
    static native boolean nativeImage(long page, long queue, long token, byte[] bytes);
    static native void nativeImageStatus(long page, long queue, long token, String retryKey, boolean success);
    private static native boolean nativeImageEvents(long page, long generation);
    private static native int[] nativeRender(long page, int width, int height, float scale, boolean rebuild, boolean caretVisible, String offsets, long epoch, long time);
    private static native void nativeAnimationPark(long page, long time);
    private static native long nativeAnimationPresented(long page, long epoch, long generation, long time);
    private static native String nativeSnapshot(long page);
    private static native String nativeInput(long page, String request);
    private static native boolean nativeClick(long page, long generation, long handle);
    private static native boolean nativeNumber(long page,long generation,long handle,double value);
    private static native boolean nativePlot(long page,long generation,String identity,int mode,float x,float y,int index);
    private static native void nativeRelease(long page);
    private static native void nativeReset(long page);
    private static native void nativeEnvironment(long page, String environment);
    private static native String nativeBeginBridge(long page, String nonce);
    private static native String nativeReceiveBridge(long page, String frame);
    private static native String nativeRequestLeave(long page, String reason);
    private static native String nativeDrainActions(long page);
    private static native long nativeAttach(android.view.Surface surface, int width, int height, long existing);
    private static native void nativeSuspend(long surface);
    private static native void nativePrepareGpu(long page, long surface, int width, int height, float scale, boolean rebuild, boolean caretVisible, String offsets, long epoch, long time);
    private static native boolean nativePresentGpu(long surface, long generation);
    private static native void nativeDetach(long surface);
    public interface PullToRefresh { boolean onTouch(MotionEvent event); }
    private PullToRefresh pullToRefresh;
    public void setPullToRefresh(PullToRefresh callback) { pullToRefresh = callback; }
    public void clearPullToRefresh(PullToRefresh callback) { if (pullToRefresh == callback) pullToRefresh = null; }
    @Override public boolean canScrollVertically(int direction) {
        return surface != null && (surface.longPressed || selectionHandles.dragging() || sliders.isDragging()
            || scrolling.canScroll(surface.scrollTarget, direction, 1));
    }
    public interface Listener {
        void onLoaded();
        void onFrame(String metadata);
        void onError(Throwable error);
        default void onMessage(long generation, String frame) { }
        default boolean onRestart(long generation) { return false; }
        default void onImage(String source, java.util.function.Consumer<byte[]> result) { result.accept(null); }
        default Runnable onImageRequest(String source, java.util.function.Consumer<byte[]> result) { onImage(source, result); return () -> {}; }
    }
    private final ExecutorService worker;
    private final android.os.Handler main = new android.os.Handler(android.os.Looper.getMainLooper());
    private final GpuBootstrap bootstrap = new GpuBootstrap(main);
    private final GpuRecovery recovery = new GpuRecovery(this);
    private TextView bootstrapMessage;
    private int bootstrapSurfaceImportance;
    private final Listener listener;
    private String source;
    private long nativePage;
    private UiSurface surface;
    private final GpuIme ime;
    private volatile long inputPageGeneration;
    private final View feedback;
    private boolean active, destroyed, ready, busy, pending;
    private boolean sessionFailed;
    private Runnable pendingClick;
    private final GpuSlider sliders=new GpuSlider();
    private boolean numberBusy;
    private int surfaceGeneration;
    private final GpuAnimationScheduler animationScheduler = new GpuAnimationScheduler(new GpuAnimationScheduler.Host() {
        public long now() { return android.os.SystemClock.uptimeMillis(); }
        public void post(Runnable callback, long delayMillis) { postOnAnimationDelayed(callback, delayMillis); }
        public void remove(Runnable callback) { removeCallbacks(callback); }
    }, () -> scheduleFrame());
    private int surfaceEpoch;
    private boolean hasSurfaceFrame;
    private Bitmap bitmap;
    private JSONObject snapshot;
    private float frameScale;
    private long gpuSurface;
    private boolean attaching;
    private java.util.function.Consumer<Boolean> nextPresentation;
    private final java.util.ArrayList<Runnable> surfaceRedraws = new java.util.ArrayList<>();
    private int unoccludedWidth, unoccludedHeight;
    private boolean gpuAttached;
    private String frameEnvironment;
    private final android.graphics.Rect chromeInsets = new android.graphics.Rect();
    private boolean viewDirty = true;
    private final GpuScroll scrolling = new GpuScroll();
    private final GpuReach reaches = new GpuReach();
    private final GpuImages images = new GpuImages();
    private final GpuAccessibilityIds accessibilityIds = new GpuAccessibilityIds();
    private GpuTextMenu textMenu;
    private GpuSelectionHandles selectionHandles;
    private android.content.res.Configuration hostConfiguration;
    private org.json.JSONArray textScale;
    private float textScalePreference = Float.NaN, textScaleDensity = Float.NaN;
    private final GpuEnvironment hostEnvironment = new GpuEnvironment();
    private final boolean logic;
    private boolean bridgeReady;
    private long bridgeGeneration;
    private long workerGeneration;

    public GpuView(Context context, String source, ExecutorService worker, Listener listener) {
        this(context, source, worker, listener, false);
    }
    public GpuView(Context context, String source, ExecutorService worker, Listener listener, boolean logic) {
        super(context);
        hostConfiguration = new android.content.res.Configuration(context.getResources().getConfiguration());
        this.logic = logic;
        this.worker = worker; this.listener = listener; this.source = source;
        // SurfaceView overlays can be hidden by its surface hole on physical devices.
        feedback = new View(context);
        feedback.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        surface = new UiSurface();
        ime = new GpuIme(surface,new GpuIme.Host() {
            @Override public void showTextMenu() { if (textMenu != null) textMenu.show(); }
            @Override public void hideTextMenu() { if (textMenu != null) textMenu.dismiss(); if (selectionHandles != null) selectionHandles.update(); }
            @Override public void editingChanged() {
                if (textMenu != null) textMenu.update();
                if (selectionHandles != null) {
                    selectionHandles.setBounds(0,0,feedback.getWidth(),feedback.getHeight());
                    selectionHandles.update();
                }
            }
            @Override public void request(JSONObject request,java.util.function.Consumer<JSONObject> reply) {
                final long pageGeneration=inputPageGeneration;
                worker.execute(() -> {
                    if (pageGeneration != inputPageGeneration) return;
                    JSONObject result=null;
                    try {
                        if (nativePage != 0) {
                            Object value=new org.json.JSONTokener(nativeInput(nativePage,request.toString())).nextValue();
                            if (value instanceof JSONObject) result=(JSONObject)value;
                        }
                    } catch (Throwable error) {
                        // Reject later queued edits against the failed mirror before they run.
                        if ("edit".equals(request.optString("op"))) {
                            try { nativeInput(nativePage,new JSONObject().put("op","blur").put("identity",request.optString("identity")).put("token",request.optLong("token")).toString()); }
                            catch (Throwable ignored) { }
                        }
                        main.post(() -> { if (pageGeneration == inputPageGeneration) showError(error); });
                    } finally {
                        // Draining actions invalidates View. Read-only hit queries must retain
                        // their frame generation until the following focus request is queued.
                        if (logic && nativePage != 0 && "edit".equals(request.optString("op"))
                                && (result == null || result.optBoolean("needs_view_render"))) {
                            try { publishBridgeUpdate(workerGeneration,nativeDrainActions(nativePage)); }
                            catch (Throwable error) { main.post(() -> { if (pageGeneration == inputPageGeneration) showError(error); }); }
                        }
                    }
                    final JSONObject response=result;
                    main.post(() -> { if (!destroyed && pageGeneration == inputPageGeneration) reply.accept(response); });
                });
            }
            @Override public void render(boolean rebuild) {
                if (destroyed) return;
                viewDirty |= rebuild; scheduleFrame();
            }
        });
        textMenu=new GpuTextMenu(surface,ime,() -> {
            JSONArray nodes=snapshot == null ? null : snapshot.optJSONArray("nodes");
            for (int i=0; nodes != null && i<nodes.length(); i++) {
                JSONObject node=nodes.optJSONObject(i), input=node == null ? null : node.optJSONObject("input");
                if (input != null && input.optLong("edit_token") > 0) return surface.bounds(node);
            }
            return new Rect();
        });
        selectionHandles=new GpuSelectionHandles(ime,new GpuSelectionHandles.Host() {
            public JSONObject frame() { return snapshot; }
            public float scale() { return frameScale; }
            public boolean busy() { return busy; }
            public void hideMenu() { textMenu.dismiss(); }
            public void showMenu() { textMenu.show(); }
        });
        feedback.getOverlay().add(selectionHandles);
        ime.setCaretActive(false);
        surface.getHolder().addCallback(this);
        addView(surface, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        addView(feedback, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        reload();
    }

    public void reload() {
        reload(source);
    }
    public void reload(String source) {
        if (destroyed) return;
        animationScheduler.cancel();
        clearBootstrap();
        recovery.clear();
        this.source = source;
        pendingClick = null;
        final String module = source;
        final long pageGeneration = ++inputPageGeneration;
        ime.reset();
        ready = false; sessionFailed = false; snapshot = null; surfaceGeneration++; surface.resetFocus();
        surface.stopScroll(); scrolling.clear(); reaches.clear(); images.clear(); viewDirty = true;
        bridgeReady = false; bridgeGeneration = 0;
        if (surface.getParent() != this) {
            removeAllViews();
            addView(surface, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            addView(feedback, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        }
        worker.execute(() -> {
            try {
                workerGeneration = 0;
                if (nativePage != 0) nativeRelease(nativePage);
                nativePage = 0;
                nativePage = nativeLoad(module);
                main.post(() -> { if (!destroyed && pageGeneration == inputPageGeneration) { ready = true; if (listener != null) listener.onLoaded(); requestFrame(); } });
            } catch (Throwable error) { main.post(() -> { if (pageGeneration == inputPageGeneration) showError(error); }); }
        });
    }

    private void postError(Throwable error, long pageGeneration) {
        main.post(() -> { if (!destroyed && pageGeneration == inputPageGeneration) showError(error); });
    }

    private void showError(Throwable error) {
        if (destroyed) return;
        finishPresentation(false);
        clearBootstrap();
        ime.setCaretActive(false);
        animationScheduler.cancel();
        pending = false;
        Log.e("LingXia.UI", "Render failed", error);
        if (listener != null) listener.onError(error);
        // Retry a candidate against the current session. Reloading here would lose
        // local state and bypass the host's Logic bridge reconnection lifecycle.
        GpuRecovery.Kind kind = GpuRecovery.kind(error);
        boolean retryable = kind == GpuRecovery.Kind.SESSION ? ready || sessionFailed
                : ready && (!logic || bridgeReady);
        if (kind == GpuRecovery.Kind.SESSION) {
            sessionFailed |= ready;
            ready = false;
            surfaceGeneration++;
            inputPageGeneration++;
            pendingClick = null;
            sliders.cancel();
            ime.reset();
        }
        recovery.show(snapshot != null, kind, retryable ? () -> {
            if (destroyed) return;
            if (kind == GpuRecovery.Kind.SESSION) {
                if (!logic) reload();
                else if (listener == null || !listener.onRestart(bridgeGeneration))
                    recovery.show(snapshot != null, kind, null);
            } else {
                viewDirty = true;
                scheduleFrame();
            }
        } : null);
    }

    public void resume() { active = true; updateSurfaceVisibility(); requestFrame(); pumpImages(); }
    @Override protected void onVisibilityChanged(View changedView, int visibility) {
        super.onVisibilityChanged(changedView, visibility);
        updateSurfaceVisibility();
        if (ready && active && isShown()) scheduleFrame();
    }
    @Override protected void onWindowVisibilityChanged(int visibility) {
        super.onWindowVisibilityChanged(visibility);
        if (visibility != View.VISIBLE) parkAnimations();
        else if (ready && active && isShown()) scheduleFrame();
    }
    private void updateSurfaceVisibility() {
        if (surface == null) return;
        // A retained page must hide its separate compositor layer as well as its view tree.
        surface.setVisibility(active && isShown() ? View.VISIBLE : View.INVISIBLE);
        if (!active || !isShown()) parkAnimations();
        if (ime != null) updateCaretActivity();
    }
    private void updateCaretActivity() {
        ime.setCaretActive(active && isShown() && !destroyed && hasWindowFocus() && surface.getHolder().getSurface().isValid());
    }
    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (ime != null) {
            if (!focused) { surface.cancelRefreshGesture(); pendingClick = null; surface.cancelKeyboardNavigation(); sliders.cancel(); surface.cancelPress(); surface.clearAccessibilityHover(); }
            updateCaretActivity();
        }
    }
    public long getConnectionGeneration() { return bridgeGeneration; }
    public void afterNextPresentation(java.util.function.Consumer<Boolean> callback) { nextPresentation = callback; }
    private void finishPresentation(boolean success) {
        finishSurfaceRedraws();
        java.util.function.Consumer<Boolean> callback = nextPresentation; nextPresentation = null;
        if (callback != null) callback.accept(success);
    }
    public void captureFrame(java.util.function.Consumer<Bitmap> callback) {
        if (android.os.Build.VERSION.SDK_INT < 24 || destroyed || snapshot == null || !hasSurfaceFrame
                || surface.getWidth() == 0 || surface.getHeight() == 0 || !surface.getHolder().getSurface().isValid()) {
            callback.accept(null); return;
        }
        Bitmap capture = Bitmap.createBitmap(surface.getWidth(), surface.getHeight(), Bitmap.Config.ARGB_8888);
        final int capturedEpoch = surfaceEpoch;
        try {
            android.view.PixelCopy.request(surface, capture, result -> {
                if (result == android.view.PixelCopy.SUCCESS && !destroyed && capturedEpoch == surfaceEpoch) {
                    // The swapchain is opaque; its alpha channel is not window coverage.
                    capture.setHasAlpha(false);
                    callback.accept(capture);
                }
                else { capture.recycle(); callback.accept(null); }
            }, main);
        } catch (IllegalArgumentException error) { capture.recycle(); callback.accept(null); }
    }
    public void captureFrameWhenReady(java.util.function.Consumer<Bitmap> callback) {
        captureFrame(frame -> {
            if (frame != null || destroyed || snapshot == null || !active) {
                callback.accept(frame);
                return;
            }
            // A chrome update may resize the surface before the navigation
            // callback arrives. Wait for that buffer instead of animating a hole.
            afterNextPresentation(success -> {
                if (success) captureFrame(callback); else callback.accept(null);
            });
            requestFrame();
        });
    }
    public void pause() { surface.cancelRefreshGesture(); finishSurfaceRedraws(); surface.cancelKeyboardNavigation(); sliders.cancel(); active = false; updateSurfaceVisibility(); pendingClick = null; ime.clear(true); surfaceGeneration++; surface.cancelPress(); surface.stopScroll(); surface.clearAccessibilityHover(); }
    public void park() {
        if (destroyed) return;
        clearBootstrap();
        recovery.clear();
        final long pageGeneration = ++inputPageGeneration;
        pause(); snapshot = null; surface.resetFocus(); scrolling.clear(); reaches.clear(); images.clear(); viewDirty = true;
        bridgeReady = false; bridgeGeneration = 0;
        worker.execute(() -> {
            workerGeneration = 0;
            try { if (nativePage != 0) nativeReset(nativePage); }
            catch (Throwable error) { postError(error, pageGeneration); }
        });
    }

    public void connect(long generation, String nonce) {
        if (destroyed || generation <= bridgeGeneration) return;
        clearBootstrap();
        recovery.clear();
        bridgeGeneration = generation; bridgeReady = false; surfaceGeneration++;
        bootstrap.begin(() -> {
            if (destroyed || generation != bridgeGeneration || bridgeReady) return;
            bootstrapMessage = new TextView(getContext());
            bootstrapMessage.setText("This page is taking longer to load. You can go back and try again.");
            bootstrapMessage.setTextColor(android.graphics.Color.BLACK);
            bootstrapMessage.setBackgroundColor(android.graphics.Color.WHITE);
            bootstrapMessage.setPadding(32,48,32,32);
            bootstrapMessage.setClickable(true);
            bootstrapSurfaceImportance = surface.getImportantForAccessibility();
            surface.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
            addView(bootstrapMessage, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            Log.e("LingXia.UI", "UI_BOOTSTRAP_TIMEOUT: bridge ready and initial state not received within 5 seconds");
        });
        worker.execute(() -> {
            try {
                workerGeneration = generation;
                String hello = nativeBeginBridge(nativePage, nonce);
                main.post(() -> {
                    if (!destroyed && generation == bridgeGeneration && listener != null) listener.onMessage(generation, hello);
                });
            } catch (Throwable error) { main.post(() -> { if (generation == bridgeGeneration) showError(error); }); }
        });
    }

    public void receive(long generation, String frame) {
        if (destroyed || generation != bridgeGeneration) return;
        worker.execute(() -> {
            if (workerGeneration != generation) return;
            try { publishBridgeUpdate(generation, nativeReceiveBridge(nativePage, frame)); }
            catch (Throwable error) { main.post(() -> { if (generation == bridgeGeneration) showError(error); }); }
        });
    }

    public void requestLeave(long generation, String reason) {
        if (destroyed || generation != bridgeGeneration) return;
        worker.execute(() -> {
            if (workerGeneration != generation || nativePage == 0) return;
            try { publishBridgeUpdate(generation, nativeRequestLeave(nativePage, reason)); }
            catch (Throwable error) { main.post(() -> { if (!destroyed && generation == bridgeGeneration) showError(error); }); }
        });
    }

    private void publishBridgeUpdate(long generation, String encoded) throws org.json.JSONException {
        JSONObject update = new JSONObject(encoded);
        JSONArray outbound = update.getJSONArray("outbound"), errors = update.getJSONArray("errors");
        for (int i = 0; i < errors.length(); i++) Log.e("LingXia.UI", errors.getString(i));
        final boolean canRender = update.getBoolean("ready"), changed = update.getBoolean("changed");
        main.post(() -> {
            if (destroyed || generation != bridgeGeneration) return;
            try {
                // State acknowledgements leave the View before any rendering attempt.
                for (int i = 0; i < outbound.length(); i++) if (listener != null) listener.onMessage(generation, outbound.get(i).toString());
                bridgeReady = canRender;
                if (canRender) clearBootstrap();
                if (changed) requestFrame();
            } catch (Throwable error) { showError(error); }
        });
    }
    private void clearBootstrap() {
        bootstrap.cancel();
        if (bootstrapMessage != null) {
            removeView(bootstrapMessage);
            bootstrapMessage = null;
            surface.setImportantForAccessibility(bootstrapSurfaceImportance);
        }
    }
    @Override public void onConfigurationChanged(android.content.res.Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (configuration.screenWidthDp != hostConfiguration.screenWidthDp
                || configuration.screenHeightDp != hostConfiguration.screenHeightDp) {
            unoccludedWidth = unoccludedHeight = 0;
        }
        hostConfiguration = new android.content.res.Configuration(configuration);
        textScale = null;
        surface.stopScroll();
        surface.clearFeedback(); surface.requestApplyInsets(); requestFrame();
    }
    @Override public android.view.WindowInsets onApplyWindowInsets(android.view.WindowInsets insets) {
        main.post(this::requestFrame);
        return super.onApplyWindowInsets(insets);
    }
    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        super.onMeasure(widthSpec, heightSpec);
        int width = getMeasuredWidth(), height = getMeasuredHeight();
        android.view.WindowInsets insets = android.os.Build.VERSION.SDK_INT >= 23 ? getRootWindowInsets() : null;
        boolean keyboard = insets != null && (android.os.Build.VERSION.SDK_INT >= 30
            ? insets.isVisible(android.view.WindowInsets.Type.ime())
            : insets.getSystemWindowInsetBottom() > insets.getStableInsetBottom());
        if (!keyboard || width != unoccludedWidth) {
            unoccludedWidth = width;
            unoccludedHeight = height;
        }
        // Keep the compositor buffer at its unoccluded size during IME transitions.
        // The parent clips it; View.env.keyboard drives the content's usable height.
        int surfaceHeight = Math.max(height, unoccludedHeight);
        surface.measure(MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
            MeasureSpec.makeMeasureSpec(surfaceHeight, MeasureSpec.EXACTLY));
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        super.onLayout(changed, left, top, right, bottom);
        // Host feedback children can relayout without changing the content viewport.
        if (changed) requestFrame();
    }

    public void updateEnvironment(String update) {
        if (destroyed) return;
        try {
            if (hostEnvironment.update(new JSONObject(update))) requestFrame();
        } catch (Exception error) { Log.e("LingXia.UI", "Invalid host environment", error); }
    }

    /** Measured overlap in physical pixels, supplied by the native host after layout. */
    public void setChromeInsets(int left, int top, int right, int bottom) {
        if (destroyed || (chromeInsets.left == left && chromeInsets.top == top
                && chromeInsets.right == right && chromeInsets.bottom == bottom)) return;
        chromeInsets.set(left, top, right, bottom);
        recovery.setInsets(chromeInsets);
        requestFrame();
    }

    private String measureEnvironment(float scale) {
        try {
            int top = 0, right = 0, bottom = 0, left = 0, keyboard = 0;
            android.view.WindowInsets windowInsets = android.os.Build.VERSION.SDK_INT >= 23 ? getRootWindowInsets() : null;
            if (windowInsets != null) {
                if (android.os.Build.VERSION.SDK_INT >= 30) {
                    android.graphics.Insets bars = windowInsets.getInsets(
                        android.view.WindowInsets.Type.systemBars() | android.view.WindowInsets.Type.displayCutout());
                    top = bars.top; right = bars.right; bottom = bars.bottom; left = bars.left;
                    keyboard = windowInsets.getInsets(android.view.WindowInsets.Type.ime()).bottom;
                } else {
                    top = windowInsets.getStableInsetTop(); right = windowInsets.getStableInsetRight();
                    bottom = windowInsets.getStableInsetBottom(); left = windowInsets.getStableInsetLeft();
                    if (windowInsets.getSystemWindowInsetBottom() > bottom)
                        keyboard = windowInsets.getSystemWindowInsetBottom();
                    if (android.os.Build.VERSION.SDK_INT >= 28 && windowInsets.getDisplayCutout() != null) {
                        android.view.DisplayCutout cutout = windowInsets.getDisplayCutout();
                        top = Math.max(top, cutout.getSafeInsetTop()); right = Math.max(right, cutout.getSafeInsetRight());
                        bottom = Math.max(bottom, cutout.getSafeInsetBottom()); left = Math.max(left, cutout.getSafeInsetLeft());
                    }
                }
            }
            // Subtract space already reserved by the host; insets describe only this surface's overlap.
            int[] origin = new int[2], rootOrigin = new int[2];
            surface.getLocationOnScreen(origin); getRootView().getLocationOnScreen(rootOrigin);
            int x = origin[0] - rootOrigin[0], y = origin[1] - rootOrigin[1];
            int trailingX = getRootView().getWidth() - x - surface.getWidth();
            int trailingY = getRootView().getHeight() - y - surface.getHeight();
            JSONObject insets = new JSONObject()
                .put("top", Math.min(surface.getHeight(), Math.max(chromeInsets.top, top - y)) / scale)
                .put("left", Math.min(surface.getWidth(), Math.max(chromeInsets.left, left - x)) / scale)
                .put("right", Math.min(surface.getWidth(), Math.max(chromeInsets.right, right - trailingX)) / scale)
                .put("bottom", Math.min(surface.getHeight(), Math.max(chromeInsets.bottom, bottom - trailingY)) / scale);
            android.content.res.Configuration configuration = hostConfiguration;
            boolean dark = (configuration.uiMode & android.content.res.Configuration.UI_MODE_NIGHT_MASK)
                == android.content.res.Configuration.UI_MODE_NIGHT_YES;
            java.util.Locale locale = android.os.Build.VERSION.SDK_INT >= 24
                ? configuration.getLocales().get(0) : configuration.locale;
            float preference = getResources().getConfiguration().fontScale;
            float density = getResources().getDisplayMetrics().density;
            // A retained page can move between Activities without a View config callback.
            if (textScale == null || textScalePreference != preference || textScaleDensity != density) {
                textScale = GpuTextScale.measure(getResources());
                textScalePreference = preference;
                textScaleDensity = density;
            }
            return hostEnvironment.apply(new JSONObject().put("theme", dark ? "dark" : "light").put("insets", insets)
                .put("textScale", textScale)
                .put("keyboard", new JSONObject().put("height", Math.min(surface.getHeight(), Math.max(0, keyboard - trailingY)) / scale))
                .put("os", "android").put("formFactor", configuration.smallestScreenWidthDp >= 600 ? "tablet" : "phone")
                .put("displayLanguage", locale.toLanguageTag()).put("aside", false).put("runner", false)).toString();
        } catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native environment", error); }
    }
    public void dispose() {
        if (destroyed) return;
        animationScheduler.cancel();
        nextPresentation = null;
        finishSurfaceRedraws();
        clearBootstrap();
        recovery.clear();
        surface.cancelKeyboardNavigation(); sliders.cancel(); destroyed = true; images.close(); inputPageGeneration++; ime.clear(false); surfaceGeneration++; surface.stopScroll();
        pendingClick = null;
        if (gpuSurface != 0) { nativeDetach(gpuSurface); gpuSurface = 0; }
        worker.execute(() -> { if (nativePage != 0) { nativeRelease(nativePage); nativePage = 0; } });
        if (bitmap != null) { bitmap.recycle(); bitmap = null; }
    }
    @Override public void surfaceCreated(SurfaceHolder holder) { updateCaretActivity(); }
    @Override public void surfaceDestroyed(SurfaceHolder holder) {
        surface.cancelRefreshGesture();
        parkAnimations();
        hasSurfaceFrame = false; surfaceEpoch++;
        finishSurfaceRedraws();
        ime.setCaretActive(false);
        surfaceGeneration++; surface.cancelKeyboardNavigation(); sliders.cancel(); surface.cancelPress();
        gpuAttached = false;
        if (gpuSurface != 0) nativeSuspend(gpuSurface);
    }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        hasSurfaceFrame = false; surfaceEpoch++;
        surfaceGeneration++;
        if (!gpuAttached) attachGpu();
        requestFrame();
    }

    @Override public void surfaceRedrawNeeded(SurfaceHolder holder) { requestFrame(); }
    @Override public void surfaceRedrawNeededAsync(SurfaceHolder holder, Runnable finished) {
        if (destroyed || !active || !holder.getSurface().isValid()) { finished.run(); return; }
        surfaceRedraws.add(finished);
        requestFrame();
    }
    private void finishSurfaceRedraws() {
        if (surfaceRedraws.isEmpty()) return;
        java.util.ArrayList<Runnable> callbacks = new java.util.ArrayList<>(surfaceRedraws);
        surfaceRedraws.clear();
        for (Runnable callback : callbacks) callback.run();
    }

    private void attachGpu() {
        if (attaching || destroyed || !active || !isShown() || !surface.getHolder().getSurface().isValid()) return;
        final int requested = surfaceGeneration, width = surface.getWidth(), height = surface.getHeight();
        if (width <= 0 || height <= 0) return;
        final android.view.Surface target = surface.getHolder().getSurface();
        final long existing = gpuSurface;
        attaching = true;
        worker.execute(() -> {
            long created = 0;
            try { created = nativeAttach(target, width, height, existing); }
            catch (Throwable error) { Log.e("LingXia.UI", "GPU unavailable; using CPU reference renderer", error); }
            final long result = created;
            main.post(() -> {
                attaching = false;
                if (destroyed || requested != surfaceGeneration) {
                    if (result != 0) {
                        if (destroyed) nativeDetach(result);
                        else { gpuSurface = result; nativeSuspend(result); }
                    }
                    if (!destroyed) attachGpu();
                    return;
                }
                gpuSurface = result;
                gpuAttached = result != 0;
                Log.i("LingXia.UI", "backend=" + (result != 0 ? "vello-vulkan" : "vello-cpu") + " surface=" + result);
                requestFrame();
            });
        });
    }

    private void requestFrame() {
        viewDirty = true;
        scheduleFrame();
    }
    private void parkAnimations() {
        if (worker == null || destroyed) return;
        animationScheduler.cancel();
        final long time = android.os.SystemClock.uptimeMillis();
        worker.execute(() -> {
            try { if (nativePage != 0) nativeAnimationPark(nativePage, time); }
            catch (Throwable error) { Log.e("LingXia.UI", "Animation pause failed", error); }
        });
    }
    private void acknowledgeAnimation(int epoch, long generation) {
        final long time = android.os.SystemClock.uptimeMillis();
        worker.execute(() -> {
            try {
                if (nativePage == 0) return;
                final long delay = nativeAnimationPresented(nativePage, epoch, generation, time);
                main.post(() -> {
                    if (delay < 0 || destroyed || !active || !isShown() || getWindowVisibility() != View.VISIBLE
                        || epoch != surfaceGeneration || snapshot == null || snapshot.optLong("generation") != generation
                        || busy || pending) return;
                    animationScheduler.schedule(delay, time);
                });
            } catch (Throwable error) { Log.e("LingXia.UI", "Animation presentation acknowledgement failed", error); }
        });
    }
    private void scheduleFrame() {
        animationScheduler.cancel();
        pending = true;
        if (!active || destroyed || !ready || !isShown() || getWindowVisibility() != View.VISIBLE || (logic && !bridgeReady) || !surface.getHolder().getSurface().isValid()) return;
        final int width = surface.getWidth(), height = surface.getHeight();
        if (width <= 0 || height <= 0) return;
        final float scale = getResources().getDisplayMetrics().density;
        final String environment = measureEnvironment(scale);
        if (!environment.equals(frameEnvironment)) { frameEnvironment = environment; surfaceGeneration++; viewDirty = true; }
        if (busy || attaching || numberBusy) return;
        final int requested = surfaceGeneration;
        final long targetSurface = gpuSurface;
        final boolean rebuild = viewDirty || snapshot == null;
        final long scrollRevision = scrolling.revision;
        final GpuIme.Ticket inputTicket=ime.capture();
        final boolean caretVisible=ime.caretVisible();
        final java.util.Map<String, Double> capturedOffsets = scrolling.snapshot();
        final String offsets = new JSONObject(capturedOffsets).toString();
        viewDirty = false;
        pending = false; busy = true;
        final long queuedAt = System.nanoTime();
        final long animationTime = android.os.SystemClock.uptimeMillis();
        worker.execute(() -> {
            try {
                long started = System.nanoTime();
                final long workerQueueUs = (started - queuedAt) / 1000;
                nativeEnvironment(nativePage, environment);
                Bitmap rendered = null;
                if (targetSurface != 0) nativePrepareGpu(nativePage, targetSurface, width, height, scale, rebuild, caretVisible, offsets, requested, animationTime);
                else rendered = Bitmap.createBitmap(nativeRender(nativePage, width, height, scale, rebuild, caretVisible, offsets, requested, animationTime), width, height, Bitmap.Config.ARGB_8888);
                if (logic) {
                    JSONObject committed = new JSONObject(nativeDrainActions(nativePage));
                    committed.put("changed", committed.optBoolean("localChanged"));
                    publishBridgeUpdate(workerGeneration, committed.toString());
                }
                final Bitmap next = rendered;
                String metadata = nativeSnapshot(nativePage);
                JSONObject nextSnapshot = new JSONObject(metadata);
                long elapsed = (System.nanoTime() - started) / 1000000;
                final long preparedAt = System.nanoTime();
                main.post(() -> Choreographer.getInstance().postFrameCallback(time -> {
                    final long presentStarted = System.nanoTime();
                    busy = false;
                    boolean presented = false;
                    if (!destroyed && active && isShown() && getWindowVisibility() == View.VISIBLE && requested == surfaceGeneration && surface.getHolder().getSurface().isValid()) {
                        if (surface.present(next, nextSnapshot)) {
                            presented = true;
                            hasSurfaceFrame = true;
                            boolean contentChanged=snapshot == null || !snapshot.optJSONArray("nodes").toString().equals(nextSnapshot.optJSONArray("nodes").toString());
                            if (bitmap != null) bitmap.recycle();
                            JSONObject previousSnapshot=snapshot;
                            bitmap = next; surface.acceptFrame(nextSnapshot,scale);
                            // Repainting an old frame (caret/scroll) is not recovery.
                            if (rebuild) recovery.clear();
                            acknowledgeAnimation(requested, nextSnapshot.optLong("generation"));
                            ime.presented(nextSnapshot,inputTicket);
                            if (contentChanged) {
                                GpuTextAccessibility.events(surface,previousSnapshot,nextSnapshot,accessibilityIds::id);
                                for(int index:GpuSlider.changed(previousSnapshot,nextSnapshot))surface.accessibilityEvent(accessibilityIds.id(index),AccessibilityEvent.TYPE_VIEW_SELECTED);
                            }
                            updateCaretActivity();
                            scrolling.accept(nextSnapshot, scrollRevision, capturedOffsets);
                            dispatchReach(nextSnapshot.optLong("generation"), reaches.presented(nextSnapshot), reaches.pending);
                            updateImages(nextSnapshot.optJSONArray("images"));
                            if (contentChanged) surface.accessibilityEvent(View.NO_ID,AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
                            if (listener != null) listener.onFrame(metadata);
                            Log.i("LingXia.UI", "present generation=" + snapshot.optLong("generation") + " surface=" + targetSurface + " size=" + width + "x" + height + " workerMs=" + elapsed + " workerQueueUs=" + workerQueueUs + " uiWaitUs=" + ((presentStarted - preparedAt) / 1000) + " uiUs=" + ((System.nanoTime() - presentStarted) / 1000) + " rebuild=" + rebuild + " caretVisible=" + caretVisible + " nodes=" + snapshot.optJSONArray("nodes").length() + " images=" + snapshot.optInt("loadedImages") + " metrics=" + snapshot.optJSONObject("metrics"));
                        } else if (next != null) { next.recycle(); }
                    } else if (next != null) { next.recycle(); }
                    if (presented) { finishPresentation(true); surface.finishKeyboardNavigation(); drainNumber(); }
                    drainClick(presented);
                    if (pending && !destroyed) scheduleFrame();
                }));
            } catch (Throwable error) {
                main.post(() -> { busy = false; pendingClick = null; sliders.cancel(); viewDirty = true; if (active && requested == surfaceGeneration) showError(error); if (pending && !destroyed) scheduleFrame(); });
            }
        });
    }

    private void click(int index) {
        click(index,null,false);
    }
    private void click(int index,float[] point,boolean word) { click(index,point,word,null); }
    private void click(int index,float[] point,boolean word,Integer selection) {
        JSONObject node = surface.node(index);
        if (!active || destroyed || !ready || node == null || snapshot == null || surface.bounds(node).isEmpty()) return;
        surface.cancelKeyboardNavigation(); sliders.cancel();
        if (busy) {
            final String nodes=snapshot.optJSONArray("nodes").toString();
            final int generation=surfaceGeneration;
            final float scale=frameScale;
            pendingClick=() -> {
                if (generation != surfaceGeneration || scale != frameScale || snapshot == null) return;
                if (nodes.equals(snapshot.optJSONArray("nodes").toString())) click(index,point,word,selection);
                else {
                    int rebound=GpuClick.find(node,snapshot.optJSONArray("nodes"));
                    if(rebound>=0)click(rebound,point,word,selection);
                }
            };
            return;
        }
        if (point != null) dismissPlots(GpuPlot.outsideSelections(snapshot.optJSONArray("nodes"),point[0],point[1]),surfaceGeneration,frameScale);
        if (GpuIme.editable(node)) {
            if (point == null) ime.focus(node,snapshot.optLong("generation"));
            else ime.touch(node,snapshot.optLong("generation"),point[0],point[1],word);
            return;
        }
        if (ime.isActive()) ime.clear(true);
        boolean plot = GpuPlot.enabled(node);
        int plotMode = selection != null ? 2 : point == null ? 1 : 0;
        int selected = selection == null ? -1 : selection;
        String identity = node.optString("identity");
        long handle = plot ? GpuPlot.plot(node).optLong("on_select") : node.optLong("on_click", 0);
        long generation = snapshot.optLong("generation");
        if (handle == 0) return;
        final long pageGeneration = inputPageGeneration;
        worker.execute(() -> {
            if (pageGeneration != inputPageGeneration) return;
            try {
                if (plot ? nativePlot(nativePage,generation,identity,plotMode,point == null ? 0 : point[0],point == null ? 0 : point[1],selected) : nativeClick(nativePage, generation, handle)) {
                    if (logic) publishBridgeUpdate(workerGeneration, nativeDrainActions(nativePage));
                    main.post(() -> { if (!destroyed && pageGeneration == inputPageGeneration) { surface.sendAccessibilityEvent(AccessibilityEvent.TYPE_VIEW_CLICKED); requestFrame(); } });
                }
            } catch (Throwable error) { postError(error, pageGeneration); }
        });
    }

    private void dismissPlots(java.util.List<JSONObject> targets,int surfaceVersion,float scale) {
        if(targets.isEmpty()||!active||destroyed||!ready||snapshot==null||surfaceVersion!=surfaceGeneration||scale!=frameScale)return;
        if(busy){pendingClick=()->dismissPlots(targets,surfaceVersion,scale);return;}
        java.util.List<String> identities=new java.util.ArrayList<>();
        for(JSONObject previous:targets){
            int rebound=GpuClick.find(previous,snapshot.optJSONArray("nodes"));
            if(rebound>=0)identities.add(surface.node(rebound).optString("identity"));
        }
        if(identities.isEmpty())return;
        long generation=snapshot.optLong("generation");
        final long pageGeneration = inputPageGeneration;
        worker.execute(()->{
            if (pageGeneration != inputPageGeneration) return;
            try{
                boolean changed=false;
                for(String identity:identities)changed|=nativePlot(nativePage,generation,identity,2,0,0,-1);
                if(changed){
                    if(logic)publishBridgeUpdate(workerGeneration,nativeDrainActions(nativePage));
                    main.post(()->{if(!destroyed && pageGeneration == inputPageGeneration)requestFrame();});
                }
            }catch(Throwable error){postError(error, pageGeneration);}
        });
    }

    private boolean proposeNumber(JSONObject node,double value) {
        if(!active || destroyed || !ready || node==null || !GpuSlider.enabled(node))return false;
        if(surface.bounds(node).isEmpty() && !node.optString("identity").equals(surface.pendingKeyboardIdentity))return false;
        if(ime.isActive())ime.clear(true);
        if(!sliders.propose(node,value,inputPageGeneration))return false;
        drainNumber();return true;
    }
    private void drainNumber() {
        if(busy || numberBusy || sliders.pending==null || surface.pendingKeyboardIdentity!=null)return;
        GpuSlider.Proposal proposal=sliders.pending;sliders.pending=null;
        if(!active || destroyed || proposal.page!=inputPageGeneration || proposal.epoch!=sliders.epoch)return;
        JSONArray nodes=snapshot==null?null:snapshot.optJSONArray("nodes");JSONObject target=null;
        for(int i=0;nodes!=null && i<nodes.length();i++)if(proposal.matches(nodes.optJSONObject(i))){target=nodes.optJSONObject(i);break;}
        if(target==null || surface.bounds(target).isEmpty() || proposal.value==GpuSlider.range(target).optDouble("value"))return;
        long handle=GpuSlider.range(target).optLong("on_change"),generation=snapshot.optLong("generation");
        numberBusy=true;sliders.flight=proposal;
        worker.execute(() -> {
            boolean changed=false;Throwable failure=null;
            try {
                if(proposal.page==inputPageGeneration && proposal.epoch==sliders.epoch && nativePage!=0) {
                    changed=nativeNumber(nativePage,generation,handle,proposal.value);
                    if(changed && logic)publishBridgeUpdate(workerGeneration,nativeDrainActions(nativePage));
                }
            }catch(Throwable error){failure=error;}
            final boolean applied=changed;final Throwable error=failure;
            main.post(() -> {
                numberBusy=false;sliders.flight=null;
                if(destroyed)return;
                if(error!=null){sliders.cancel();if(proposal.page==inputPageGeneration)showError(error);return;}
                if(applied)requestFrame();else if(pending)scheduleFrame();else drainNumber();
            });
        });
    }

    private void drainClick(boolean presented) {
        Runnable action=pendingClick; pendingClick=null;
        if (presented && action != null) action.run();
    }

    private void updateImages(JSONArray sources) {
        images.update(sources == null ? new JSONArray() : sources); pumpImages(); flushImageEvents();
    }
    private void flushImageEvents() {
        if (destroyed || snapshot == null) return;
        final long page = nativePage, generation = snapshot.optLong("generation");
        worker.execute(() -> {
            try {
                if (nativePage != page || page == 0) return;
                if (nativeImageEvents(page, generation)) {
                    if (logic) publishBridgeUpdate(workerGeneration, nativeDrainActions(page));
                    main.post(() -> { if (!destroyed && nativePage == page) requestFrame(); });
                }
            } catch (Throwable error) { main.post(() -> { if (!destroyed && nativePage == page) showError(error); }); }
        });
    }
    private void pumpImages() {
        if (destroyed || !active || listener == null) return;
        for (GpuImages.Request request : images.start()) {
            final long page = nativePage;
            Runnable cancel = listener.onImageRequest(request.source, bytes -> main.post(() -> {
                if (destroyed || !images.current(request)) {
                    images.complete(request); pumpImages(); return;
                }
                worker.execute(() -> {
                    boolean changed = false;
                    try { if (nativePage == page && bytes != null) changed = images.apply(page, request, bytes); }
                    catch (Throwable error) { Log.w("LingXia.UI", "Image failed: " + request.source, error); }
                    if (nativePage == page) images.status(page, request, changed);
                    final boolean updated = changed;
                    main.post(() -> {
                        boolean current = images.complete(request);
                        if (!destroyed && current && updated) scheduleFrame();
                        if (!destroyed && current) flushImageEvents();
                        pumpImages();
                    });
                });
            }));
            images.track(request, cancel);
        }
    }

    private void dispatchReach(long generation, java.util.List<Long> handles, boolean more) {
        if (handles.isEmpty() && !more) return;
        final long pageGeneration = inputPageGeneration;
        // Queue before any next preparation, so generation validation sees the presented frame.
        worker.execute(() -> {
            if (pageGeneration != inputPageGeneration) return;
            try {
                boolean changed = false;
                for (long handle : handles) changed |= nativeClick(nativePage, generation, handle);
                if (logic && changed) publishBridgeUpdate(workerGeneration, nativeDrainActions(nativePage));
                if (changed || more) main.post(() -> { if (!destroyed && pageGeneration == inputPageGeneration) requestFrame(); });
            } catch (Throwable error) { postError(error, pageGeneration); }
        });
    }

    private final class UiSurface extends SurfaceView {
        private int pressed = -1, accessibilityFocus = -1;
        private int accessibilityHover = -1;
        private boolean exploring;
        private float explorationX, explorationY;
        private int keyboardFocus = -1;
        private String pendingKeyboardIdentity;
        private boolean pendingKeyboardActivation;
        private long pendingKeyboardPage;
        private long pressedGeneration;
        private Runnable longPress;
        private boolean longPressed;
        private boolean touchMoved;
        private float downX, downY, lastPosition;
        private int dragAxis = 1;
        private String scrollTarget;
        private boolean dragging;
        private boolean refreshingGesture;
        private android.view.VelocityTracker velocity;
        private final android.widget.OverScroller fling = new android.widget.OverScroller(getContext());
        private int flingY;
        private final Choreographer.FrameCallback scrollFrame = new Choreographer.FrameCallback() {
            @Override public void doFrame(long time) {
                if (!active || destroyed || scrollTarget == null || !fling.computeScrollOffset()) return;
                int y = fling.getCurrY();
                boolean changed = scrolling.scrollBy(scrollTarget, (y - flingY) / frameScale);
                flingY = y;
                if (changed) scheduleFrame();
                else { fling.abortAnimation(); return; }
                Choreographer.getInstance().postFrameCallback(this);
            }
        };
        void stopScroll() {
            fling.abortAnimation();
            Choreographer.getInstance().removeFrameCallback(scrollFrame);
            if (velocity != null) { velocity.recycle(); velocity = null; }
            scrollTarget = null; dragging = false;
        }
        String scrollHit(float x, float y) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            for (int i = nodes == null ? -1 : nodes.length() - 1; i >= 0; i--) {
                JSONObject candidate = node(i), scroll = candidate.optJSONObject("scroll");
                if (scroll != null && scroll.optDouble("max_offset") > 0 && GpuClip.contains(candidate, x/frameScale, y/frameScale, true)) return scroll.optString("id");
            }
            return null;
        }
        private final GpuClip.Highlight pressedOverlay = new GpuClip.Highlight();
        private final GpuClip.Highlight focusOverlay = new GpuClip.Highlight();
        private final int slop;
        UiSurface() {
            super(GpuView.this.getContext());
            slop = ViewConfiguration.get(getContext()).getScaledTouchSlop();
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
            setFocusable(true);
            setFocusableInTouchMode(true);
            if (android.os.Build.VERSION.SDK_INT >= 26) setDefaultFocusHighlightEnabled(false);
            pressedOverlay.setColor(0x22000000);
            focusOverlay.setColor(Color.TRANSPARENT);
        }
        JSONObject node(int index) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            return nodes == null ? null : nodes.optJSONObject(index);
        }
        Rect bounds(JSONObject node) {
            JSONArray b = node.optJSONArray("visible_bounds");
            if (b == null) b = node.optJSONArray("bounds");
            if (b == null) return new Rect();
            return new Rect((int)(b.optDouble(0)*frameScale), (int)(b.optDouble(1)*frameScale),
                (int)((b.optDouble(0)+b.optDouble(2))*frameScale), (int)((b.optDouble(1)+b.optDouble(3))*frameScale));
        }
        int hit(float x, float y) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            if (nodes == null) return -1;
            for (int i = nodes.length()-1; i >= 0; i--) {
                JSONObject node = nodes.optJSONObject(i);
                if (node != null && (node.optLong("on_click",0) != 0 || GpuIme.editable(node) || GpuSlider.enabled(node) || GpuPlot.enabled(node)) && GpuClip.contains(node, x/frameScale, y/frameScale, false)) return i;
            }
            return -1;
        }
        void cancelRefreshGesture() {
            if (!refreshingGesture) return;
            refreshingGesture = false;
            long now = android.os.SystemClock.uptimeMillis();
            MotionEvent cancel = MotionEvent.obtain(now, now, MotionEvent.ACTION_CANCEL, 0, 0, 0);
            try { if (pullToRefresh != null) pullToRefresh.onTouch(cancel); }
            finally { cancel.recycle(); }
            if (getParent() != null) getParent().requestDisallowInterceptTouchEvent(false);
        }
        void cancelPress() {
            if (longPress != null) { main.removeCallbacks(longPress); longPress=null; }
            pressed = -1; feedback.getOverlay().remove(pressedOverlay);
        }
        void clearFeedback() { cancelPress(); feedback.getOverlay().remove(focusOverlay); }
        void resetFocus() { sliders.cancel(); cancelKeyboardNavigation(); clearAccessibilityHover(); clearFeedback(); keyboardFocus=-1; accessibilityFocus=-1; accessibilityIds.clear(); }
        void acceptFrame(JSONObject frame, float scale) {
            sliders.presented(frame,scale);
            if (snapshot != null && scale == frameScale
                && snapshot.optJSONArray("nodes").toString().equals(frame.optJSONArray("nodes").toString())) {
                snapshot = frame;
                if (pressed >= 0) pressedGeneration = frame.optLong("generation");
                return;
            }
            JSONObject focused = node(keyboardFocus);
            String identity = focused == null ? "" : focused.optString("identity");
            int rebound=scale==frameScale ? GpuClick.find(node(pressed),frame.optJSONArray("nodes")) : -1;
            if(rebound<0)clearFeedback();else feedback.getOverlay().remove(focusOverlay);
            keyboardFocus=-1;
            accessibilityIds.update(frame);
            snapshot = frame; frameScale = scale;
            if(rebound>=0){pressed=rebound;pressedGeneration=frame.optLong("generation");}
            int accessibleIndex=accessibilityIds.index(accessibilityFocus);
            if (accessibilityFocus>=0 && (accessibleIndex<0 || bounds(node(accessibleIndex)).isEmpty())) {
                accessibilityEvent(accessibilityFocus,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                accessibilityFocus=-1;
            }
            if (exploring) updateAccessibilityHover();
            JSONArray nodes = frame.optJSONArray("nodes");
            for (int i=0; !identity.isEmpty() && nodes != null && i<nodes.length(); i++) {
                JSONObject candidate = node(i);
                if (identity.equals(candidate.optString("identity")) && (candidate.optLong("on_click",0)!=0 || GpuIme.editable(candidate) || GpuSlider.enabled(candidate) || GpuPlot.enabled(candidate)) && !bounds(candidate).isEmpty()) {
                    showKeyboardFocus(i);
                    break;
                }
            }
        }
        void showKeyboardFocus(int index) {
            keyboardFocus=index;
            JSONObject candidate = node(index);
            focusOverlay.setNode(candidate,frameScale);
            focusOverlay.setStroke(Math.max(1,(int)(2*frameScale)),Color.rgb(23,32,51));
            feedback.getOverlay().add(focusOverlay);
        }
        boolean present(Bitmap candidate, JSONObject frame) {
            if (!active || !getHolder().getSurface().isValid()) return false;
            if (gpuSurface != 0) {
                try {
                    long started = System.nanoTime();
                    if (!nativePresentGpu(gpuSurface, frame.optLong("generation"))) {
                        animationScheduler.schedule(0, android.os.SystemClock.uptimeMillis());
                        return false;
                    }
                    Log.i("LingXia.UI", "gpuSubmitMs=" + ((System.nanoTime()-started)/1000000));
                    return true;
                } catch (Throwable error) { showError(error); }
                return false;
            }
            if (candidate == null) return false;
            Canvas canvas = getHolder().lockCanvas();
            if (canvas == null) {
                animationScheduler.schedule(0, android.os.SystemClock.uptimeMillis());
                return false;
            }
            try {
                canvas.drawBitmap(candidate, 0, 0, null);
            } finally { getHolder().unlockCanvasAndPost(canvas); }
            return true;
        }
        @Override public boolean onTouchEvent(MotionEvent event) {
            if (sessionFailed) return false;
            if (selectionHandles.touch(event)) {
                sliders.cancel(); cancelKeyboardNavigation(); cancelPress(); stopScroll();
                getParent().requestDisallowInterceptTouchEvent(selectionHandles.dragging());
                return true;
            }
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) refreshingGesture = false;
            boolean refreshConsumed = pullToRefresh != null && pullToRefresh.onTouch(event);
            if (refreshConsumed || refreshingGesture) {
                if (!refreshingGesture) {
                    cancelPress(); stopScroll(); sliders.cancel(); cancelKeyboardNavigation();
                    refreshingGesture = true;
                }
                if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) refreshingGesture = false;
                return true;
            }
            if (action == MotionEvent.ACTION_DOWN) {
                sliders.cancel();cancelKeyboardNavigation();
                textMenu.dismiss();
                longPressed=false;touchMoved=false;
                cancelPress();
                stopScroll();
                keyboardFocus=-1; feedback.getOverlay().remove(focusOverlay);
                pressed = hit(event.getX(),event.getY()); downX = event.getX(); downY = lastPosition = event.getY();
                scrollTarget = scrollHit(downX, downY);
                sliders.begin(node(pressed),downX/frameScale,downY/frameScale,frameScale);
                pressedGeneration = snapshot == null ? 0 : snapshot.optLong("generation");
                if (pressed < 0 && scrollTarget == null && (snapshot==null||GpuPlot.outsideSelections(snapshot.optJSONArray("nodes"),downX/frameScale,downY/frameScale).isEmpty())) return pullToRefresh != null;
                velocity = android.view.VelocityTracker.obtain(); velocity.addMovement(event);
                if (scrollTarget != null) getParent().requestDisallowInterceptTouchEvent(true);
                if (pressed >= 0) {
                    pressedOverlay.setNode(node(pressed),frameScale);
                    feedback.getOverlay().add(pressedOverlay);
                    if (GpuIme.editable(node(pressed))) {
                        longPress=() -> {
                            if (pressed < 0 || snapshot == null || snapshot.optLong("generation") != pressedGeneration) return;
                            int target=pressed; longPressed=true; cancelPress();
                            performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS);
                            click(target,new float[]{downX/frameScale,downY/frameScale},true);
                        };
                        main.postDelayed(longPress,ViewConfiguration.getLongPressTimeout());
                    }
                }
                return true;
            }
            if (velocity != null) velocity.addMovement(event);
            if (action == MotionEvent.ACTION_MOVE) {
                if (longPressed) return true;
                if(sliders.move(event.getX()/frameScale,event.getY()/frameScale,slop/frameScale)) {
                    cancelPress();proposeNumber(sliders.gesture(),GpuSlider.at(sliders.gesture(),event.getX()/frameScale));return true;
                }
                float dx = event.getX() - downX, dy = event.getY() - downY;
                if (Math.abs(dx) > slop || Math.abs(dy) > slop) touchMoved = true;
                if (!dragging && touchMoved && scrollTarget != null) {
                    int axis = Math.abs(dx) > Math.abs(dy) ? 0 : 1;
                    String target = scrolling.targetForAxis(scrollTarget, axis);
                    if (target != null) {
                        dragAxis = axis; scrollTarget = target; dragging = true; cancelPress();
                        float distance = axis == 0 ? dx : dy;
                        lastPosition = (axis == 0 ? downX : downY) + Math.copySign(slop, distance);
                    }
                }
                if (dragging) {
                    float position = dragAxis == 0 ? event.getX() : event.getY();
                    boolean changed = scrolling.scrollBy(scrollTarget, (lastPosition - position) / frameScale);
                    lastPosition = position;
                    if (changed) scheduleFrame();
                    else getParent().requestDisallowInterceptTouchEvent(false);
                } else if (touchMoved) cancelPress();
            }
            if (action == MotionEvent.ACTION_UP) {
                if(sliders.hasGesture()) {
                    JSONObject target=sliders.gesture();sliders.end();cancelPress();stopScroll();
                    proposeNumber(target,GpuSlider.at(target,event.getX()/frameScale));
                    getParent().requestDisallowInterceptTouchEvent(false);return true;
                }
                int target = pressed;
                boolean sameFrame = snapshot != null && snapshot.optLong("generation") == pressedGeneration;
                cancelPress();
                if (dragging && velocity != null) {
                    ViewConfiguration configuration = ViewConfiguration.get(getContext());
                    velocity.computeCurrentVelocity(1000, configuration.getScaledMaximumFlingVelocity());
                    int speed = (int)-(dragAxis == 0 ? velocity.getXVelocity() : velocity.getYVelocity());
                    if (Math.abs(speed) >= configuration.getScaledMinimumFlingVelocity()) {
                        Log.i("LingXia.UI", "scroll fling velocity=" + speed);
                        flingY = 0;
                        fling.fling(0, 0, 0, speed, 0, 0, -100000000, 100000000);
                        Choreographer.getInstance().postFrameCallback(scrollFrame);
                    }
                } else if (target >= 0 && sameFrame && target == hit(event.getX(),event.getY())) {
                    performClick(); click(target,new float[]{event.getX()/frameScale,event.getY()/frameScale},false);
                } else if (target < 0 && sameFrame && !touchMoved && !longPressed && hit(event.getX(),event.getY())<0 && snapshot!=null) {
                    performClick();dismissPlots(GpuPlot.outsideSelections(snapshot.optJSONArray("nodes"),event.getX()/frameScale,event.getY()/frameScale),surfaceGeneration,frameScale);
                }
                if (velocity != null) { velocity.recycle(); velocity = null; }
                dragging = false;
                getParent().requestDisallowInterceptTouchEvent(false);
            }
            if (action == MotionEvent.ACTION_CANCEL || action == MotionEvent.ACTION_POINTER_DOWN) {
                touchMoved=true;
                sliders.cancel();cancelPress(); stopScroll(); getParent().requestDisallowInterceptTouchEvent(false);
            }
            return true;
        }
        @Override public boolean performClick() { super.performClick(); return true; }
        @Override public boolean onCheckIsTextEditor() { return ime != null && ime.hasFocus(); }
        @Override public android.view.inputmethod.InputConnection onCreateInputConnection(android.view.inputmethod.EditorInfo info) {
            return ime == null ? null : ime.create(info);
        }
        @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
            if (sessionFailed) return super.onKeyDown(keyCode, event);
            if (ime.hasFocus() && keyCode != KeyEvent.KEYCODE_TAB) return ime.key(event) || super.onKeyDown(keyCode,event);
            if (ime.hasFocus()) ime.clear(true);
            JSONObject rangeNode=node(keyboardOrigin());
            if(GpuSlider.enabled(rangeNode)) {
                JSONObject range=GpuSlider.range(rangeNode);double value=sliders.value(rangeNode);
                if(keyCode==KeyEvent.KEYCODE_DPAD_LEFT || keyCode==KeyEvent.KEYCODE_DPAD_DOWN)value=GpuSlider.shift(rangeNode,value,-1);
                else if(keyCode==KeyEvent.KEYCODE_DPAD_RIGHT || keyCode==KeyEvent.KEYCODE_DPAD_UP)value=GpuSlider.shift(rangeNode,value,1);
                else if(keyCode==KeyEvent.KEYCODE_MOVE_HOME)value=range.optDouble("min");
                else if(keyCode==KeyEvent.KEYCODE_MOVE_END)value=range.optDouble("max");
                else value=Double.NaN;
                if(Double.isFinite(value)){proposeNumber(rangeNode,value);return true;}
                if(keyCode==KeyEvent.KEYCODE_ENTER || keyCode==KeyEvent.KEYCODE_SPACE || keyCode==KeyEvent.KEYCODE_DPAD_CENTER)return true;
            }
            if (keyCode==KeyEvent.KEYCODE_DPAD_LEFT || keyCode==KeyEvent.KEYCODE_DPAD_RIGHT || keyCode==KeyEvent.KEYCODE_DPAD_UP || keyCode==KeyEvent.KEYCODE_DPAD_DOWN) {
                int origin=keyboardOrigin();
                if (GpuRadio.option(node(origin))) {
                    int direction=keyCode==KeyEvent.KEYCODE_DPAD_LEFT || keyCode==KeyEvent.KEYCODE_DPAD_UP ? -1 : 1;
                    int next=GpuRadio.adjacent(snapshot.optJSONArray("nodes"),origin,direction);
                    if(next>=0) navigateKeyboard(next,true);
                    return true;
                }
            }
            if (keyCode == KeyEvent.KEYCODE_TAB || keyCode == KeyEvent.KEYCODE_DPAD_DOWN || keyCode == KeyEvent.KEYCODE_DPAD_UP) {
                int direction = event.isShiftPressed() || keyCode == KeyEvent.KEYCODE_DPAD_UP ? -1 : 1;
                if (moveKeyboardFocus(direction)) return true;
            }
            if (keyCode == KeyEvent.KEYCODE_ENTER || keyCode == KeyEvent.KEYCODE_DPAD_CENTER || keyCode == KeyEvent.KEYCODE_SPACE) {
                if (pendingKeyboardIdentity!=null) { if(event.getRepeatCount()==0)pendingKeyboardActivation=true; return true; }
                if (keyboardFocus >= 0) {
                    if (event.getRepeatCount()==0) click(keyboardFocus);
                    return true;
                }
            }
            return super.onKeyDown(keyCode,event);
        }
        @Override public boolean onKeyShortcut(int keyCode, KeyEvent event) {
            if (sessionFailed) return super.onKeyShortcut(keyCode, event);
            return ime.hasFocus() ? ime.key(event) : super.onKeyShortcut(keyCode,event);
        }
        void cancelKeyboardNavigation() { pendingKeyboardIdentity=null; pendingKeyboardActivation=false; }
        int keyboardOrigin() {
            JSONArray nodes=snapshot==null?null:snapshot.optJSONArray("nodes");
            for(int i=0;pendingKeyboardIdentity!=null && nodes!=null && i<nodes.length();i++)
                if(pendingKeyboardIdentity.equals(node(i).optString("identity")))return i;
            return keyboardFocus;
        }
        boolean keyboardCandidate(JSONObject candidate) {
            if(candidate==null || (candidate.optLong("on_click",0)==0 && !GpuIme.editable(candidate) && !GpuSlider.enabled(candidate) && !GpuPlot.enabled(candidate)))return false;
            JSONArray box=candidate.optJSONArray("bounds");
            return box!=null && box.optDouble(2)>0 && box.optDouble(3)>0 && (!bounds(candidate).isEmpty() || scrolling.canReveal(snapshot,candidate));
        }
        void navigateKeyboard(int index,boolean activate) {
            JSONObject target=node(index);if(!keyboardCandidate(target))return;
            stopScroll();pendingKeyboardIdentity=target.optString("identity");pendingKeyboardActivation=activate;pendingKeyboardPage=inputPageGeneration;
            if(scrolling.reveal(snapshot,target) || busy || pending) { scheduleFrame();return; }
            finishKeyboardNavigation();
        }
        void finishKeyboardNavigation() {
            if(pendingKeyboardIdentity==null)return;
            if(!active || destroyed || pendingKeyboardPage!=inputPageGeneration){cancelKeyboardNavigation();return;}
            if(pending || busy)return;
            JSONArray nodes=snapshot==null?null:snapshot.optJSONArray("nodes");int target=-1;
            for(int i=0;nodes!=null && i<nodes.length();i++)if(pendingKeyboardIdentity.equals(node(i).optString("identity"))){target=i;break;}
            if(target<0 || !keyboardCandidate(node(target))){cancelKeyboardNavigation();return;}
            if(scrolling.reveal(snapshot,node(target))){scheduleFrame();return;}
            if(bounds(node(target)).isEmpty()){cancelKeyboardNavigation();return;}
            boolean activate=pendingKeyboardActivation;cancelKeyboardNavigation();
            boolean changed=keyboardFocus!=target;showKeyboardFocus(target);
            if(changed)accessibilityEvent(accessibilityIds.id(target),AccessibilityEvent.TYPE_VIEW_FOCUSED);
            if(activate)click(target);
        }
        boolean moveKeyboardFocus(int direction) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            if (nodes == null || nodes.length() == 0) return false;
            int current=keyboardOrigin();
            int origin = current >= 0 ? current : (direction > 0 ? -1 : nodes.length());
            for (int step=1;step<=nodes.length();step++) {
                int index = Math.floorMod(origin + direction*step,nodes.length());
                JSONObject candidate = node(index);
                if (GpuRadio.option(node(current)) && GpuRadio.member(candidate,node(current).optString("radio_group")))continue;
                if (keyboardCandidate(candidate) && GpuRadio.tabStop(nodes,index)) {
                    navigateKeyboard(index,false);
                    return true;
                }
            }
            return false;
        }
        @Override protected void onFocusChanged(boolean focused, int direction, Rect previouslyFocusedRect) {
            super.onFocusChanged(focused,direction,previouslyFocusedRect);
            if (!focused) { sliders.cancel();cancelKeyboardNavigation(); keyboardFocus=-1; feedback.getOverlay().remove(focusOverlay); if (ime != null) ime.clear(true); }
            else if (!isInTouchMode() && (ime == null || !ime.hasFocus())) moveKeyboardFocus(direction == View.FOCUS_BACKWARD || direction == View.FOCUS_UP ? -1 : 1);
        }
        private boolean accessibleNode(JSONObject node) { return semanticNode(node) && !bounds(node).isEmpty(); }
        private boolean semanticNode(JSONObject node) {
            return node!=null && !node.optBoolean("accessibility_hidden",false) && (!node.optString("text").isEmpty() || GpuRadio.group(node) || node.has("scroll")
                || node.has("checked") || node.has("range") || node.optJSONObject("input")!=null || "image".equals(node.optString("tag"))
                || ("plot".equals(node.optString("tag")) && !node.optString("accessibility_label").isEmpty()));
        }
        private int accessibilityHit(float x,float y) {
            JSONArray nodes=snapshot==null ? null : snapshot.optJSONArray("nodes");
            if (frameScale<=0 || nodes==null) return -1;
            for(int i=nodes.length()-1;i>=0;i--) {
                JSONObject node=nodes.optJSONObject(i);
                if (accessibleNode(node) && GpuClip.contains(node,x/frameScale,y/frameScale,false)) return accessibilityIds.id(i);
            }
            return -1;
        }
        private void setAccessibilityHover(int id) {
            if (id==accessibilityHover) return;
            int old=accessibilityHover; accessibilityHover=id;
            if (id>=0) accessibilityEvent(id,AccessibilityEvent.TYPE_VIEW_HOVER_ENTER);
            if (old>=0) accessibilityEvent(old,AccessibilityEvent.TYPE_VIEW_HOVER_EXIT);
        }
        void clearAccessibilityHover() { exploring=false; setAccessibilityHover(-1); }
        private void updateAccessibilityHover() { setAccessibilityHover(accessibilityHit(explorationX,explorationY)); }
        @Override public boolean dispatchHoverEvent(MotionEvent event) {
            android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            if (!active || destroyed || manager==null || !manager.isEnabled() || !manager.isTouchExplorationEnabled()) {
                clearAccessibilityHover(); return super.dispatchHoverEvent(event);
            }
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_HOVER_ENTER:
                case MotionEvent.ACTION_HOVER_MOVE:
                    exploring=true; explorationX=event.getX(); explorationY=event.getY(); updateAccessibilityHover();
                    return accessibilityHover>=0;
                case MotionEvent.ACTION_HOVER_EXIT:
                    boolean handled=exploring; clearAccessibilityHover(); return handled;
                default: return false;
            }
        }
        private void accessibilityEvent(int id,int type) {
            android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            if (getParent()==null || manager==null || !manager.isEnabled()) return;
            AccessibilityEvent event=AccessibilityEvent.obtain(type);
            // Virtual descendants must be invalidated, not only the host node.
            if (type==AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED) event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
            event.setSource(this,id); event.setPackageName(getContext().getPackageName());
            getParent().requestSendAccessibilityEvent(this,event);
        }
        @Override public AccessibilityNodeProvider getAccessibilityNodeProvider() {
            return new AccessibilityNodeProvider() {
                @Override public AccessibilityNodeInfo findFocus(int focus) {
                    if (!active || snapshot == null || sessionFailed) return null;
                    if (focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY) return accessibilityFocus < 0 ? null : createAccessibilityNodeInfo(accessibilityFocus);
                    if (focus != AccessibilityNodeInfo.FOCUS_INPUT) return null;
                    JSONArray nodes=snapshot.optJSONArray("nodes");
                    for (int i=0;nodes != null && i<nodes.length();i++) {
                        JSONObject input=node(i).optJSONObject("input");
                        if (input != null && input.optLong("edit_token") > 0 && !bounds(node(i)).isEmpty()) return createAccessibilityNodeInfo(accessibilityIds.id(i));
                    }
                    return keyboardFocus < 0 ? null : createAccessibilityNodeInfo(accessibilityIds.id(keyboardFocus));
                }
                @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
                    if (id == View.NO_ID) {
                        AccessibilityNodeInfo root = AccessibilityNodeInfo.obtain(UiSurface.this);
                        onInitializeAccessibilityNodeInfo(root);
                        JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
                        if (nodes != null) for (int i=0;i<nodes.length();i++) if (accessibilityIds.id(i)>=0 && !GpuRadio.option(node(i)) && accessibleNode(node(i))) root.addChild(UiSurface.this,accessibilityIds.id(i));
                        return root;
                    }
                    int index=accessibilityIds.index(id);
                    JSONObject node = node(index);
                    if (node == null || node.optBoolean("accessibility_hidden",false)) return null;
                    AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain();
                    info.setSource(UiSurface.this,id); info.setParent(UiSurface.this);
                    boolean button = node.optString("tag").equals("button");
                    boolean check = node.has("checked");
                    JSONObject input=node.optJSONObject("input");
                    JSONObject scroll = node.optJSONObject("scroll");
                    if (scroll != null) {
                        info.setScrollable(scroll.optDouble("max_offset") > 0);
                        if (scroll.optDouble("offset") < scroll.optDouble("max_offset")) info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
                        if (scroll.optDouble("offset") > 0) info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
                    }
                    info.setPackageName(getContext().getPackageName()); info.setClassName(input != null ? "android.widget.EditText" : scroll != null ? (GpuScroll.axis(scroll) == 0 ? "android.widget.HorizontalScrollView" : "android.widget.ScrollView") : check ? ("switch".equals(node.optString("tag")) ? "android.widget.Switch" : "android.widget.CheckBox") : button ? "android.widget.Button" : "image".equals(node.optString("tag")) ? "android.widget.ImageView" : "android.widget.TextView");
                    info.setText(node.optString("text")); info.setVisibleToUser(active && !bounds(node).isEmpty());
                    if ("plot".equals(node.optString("tag")) || "image".equals(node.optString("tag"))) {
                        info.setClassName("android.widget.ImageView");
                        info.setContentDescription(node.optString("accessibility_label"));
                    }
                    if (android.os.Build.VERSION.SDK_INT >= 24) info.setImportantForAccessibility(true);
                    String testId = node.optString("test_id");
                    if (!testId.isEmpty()) info.setViewIdResourceName(getContext().getPackageName() + ":id/" + testId);
                    info.setEnabled(input != null ? input.optBoolean("enabled",true) : !(button || check) || node.optLong("on_click",0)!=0);
                    info.setCheckable(check); if (check) info.setChecked(node.optBoolean("checked"));
                    info.setClickable((button || check || input != null) && info.isEnabled()); info.setFocusable(button || check || input != null);
                    info.setFocused(keyboardFocus == index && !ime.hasFocus());
                    GpuTextAccessibility.populate(info,node);
                    JSONArray nodes=snapshot.optJSONArray("nodes");
                    GpuRadio.populate(info,nodes,index);
                    GpuSlider.populate(info,node);
                    GpuPlot.populate(info,node);
                    if(semanticNode(node))info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN);
                    int group=GpuRadio.parent(nodes,node);
                    if(group>=0) {
                        info.setParent(UiSurface.this,accessibilityIds.id(group));
                    }
                    if(GpuRadio.group(node))for(int i=0;i<nodes.length();i++)if(GpuRadio.member(nodes.optJSONObject(i),node.optString("identity")))info.addChild(UiSurface.this,accessibilityIds.id(i));
                    Rect rect = bounds(node); Rect relative=new Rect(rect);
                    if(group>=0){JSONArray parentBounds=nodes.optJSONObject(group).optJSONArray("bounds");relative.offset(-(int)Math.round(parentBounds.optDouble(0)*frameScale),-(int)Math.round(parentBounds.optDouble(1)*frameScale));}
                    info.setBoundsInParent(relative);
                    int[] location = new int[2]; getLocationOnScreen(location); rect.offset(location[0],location[1]); info.setBoundsInScreen(rect);
                    info.setAccessibilityFocused(accessibilityFocus == id);
                    if (sessionFailed) { info.setEnabled(false); info.setClickable(false); info.setEditable(false); }
                    info.addAction(accessibilityFocus == id ? AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS : AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
                    if (info.isClickable()) info.addAction(AccessibilityNodeInfo.ACTION_CLICK);
                    return info;
                }
                @Override public boolean performAction(int id, int action, Bundle args) {
                    if (sessionFailed && action != AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS
                            && action != AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) return false;
                    int index=accessibilityIds.index(id);
                    if (!active || destroyed || node(index) == null || node(index).optBoolean("accessibility_hidden",false)) return false;
                    cancelKeyboardNavigation();sliders.end();
                    if(action==AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId() && semanticNode(node(index))) {
                        stopScroll();boolean changed=scrolling.reveal(snapshot,node(index));if(changed)scheduleFrame();return changed || !bounds(node(index)).isEmpty();
                    }
                    if(bounds(node(index)).isEmpty())return false;
                    if(GpuSlider.range(node(index))!=null && (action==AccessibilityNodeInfo.ACTION_SCROLL_FORWARD || action==AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD || action==GpuSlider.SET_PROGRESS)) {
                        if(!GpuSlider.enabled(node(index)))return false;
                        double value;
                        if(action==GpuSlider.SET_PROGRESS) {
                            if(args==null || !args.containsKey(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE))return false;
                            value=args.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE,Float.NaN);
                        } else value=GpuSlider.shift(node(index),sliders.value(node(index)),action==AccessibilityNodeInfo.ACTION_SCROLL_FORWARD?1:-1);
                        return proposeNumber(node(index),value);
                    }
                    if (action == AccessibilityNodeInfo.ACTION_SET_TEXT || action == AccessibilityNodeInfo.ACTION_SET_SELECTION) {
                        if (!GpuIme.editable(node(index))) return false;
                        String text=null; int start=-1,end=-1;
                        if (action == AccessibilityNodeInfo.ACTION_SET_TEXT) {
                            CharSequence value=args == null ? null : args.getCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE);
                            if (value != null && value.length() > 65536) return false;
                            text=value == null ? "" : value.toString();
                        } else {
                            if (args != null) {
                                start=args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT,-1);
                                end=args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT,-1);
                            }
                        }
                        return ime.accessibilityEdit(node(index),snapshot.optLong("generation"),text,start,end);
                    }
                    JSONObject scroll = node(index).optJSONObject("scroll");
                    if (scroll != null && (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD || action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)) {
                        stopScroll();
                        double distance = (GpuScroll.axis(scroll) == 0 ? bounds(node(index)).width() : bounds(node(index)).height()) / frameScale * 0.8;
                        boolean changed = scrolling.scrollBy(scroll.optString("id"), action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD ? distance : -distance);
                        if (changed) scheduleFrame();
                        return changed;
                    }
                    if (action == AccessibilityNodeInfo.ACTION_CLEAR_SELECTION && GpuPlot.canClear(node(index))) {
                        click(index,null,false,-1);return true;
                    }
                    if (action >= GpuPlot.SELECT_ITEM && action < GpuPlot.SELECT_ITEM + 128) {
                        int selected=action-GpuPlot.SELECT_ITEM;
                        if(!GpuPlot.selectable(node(index),selected))return false;
                        click(index,null,false,selected);return true;
                    }
                    if (action == AccessibilityNodeInfo.ACTION_CLICK) { if (!GpuIme.editable(node(index)) && node(index).optLong("on_click",0)==0 && !GpuPlot.enabled(node(index))) return false; click(index); return true; }
                    if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS || action == AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) {
                        if (action == AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS && accessibilityFocus != id) return false;
                        if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS && accessibilityFocus == id) return false;
                        if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS && accessibilityFocus>=0)
                            accessibilityEvent(accessibilityFocus,AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                        accessibilityFocus = action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS ? id : -1;
                        accessibilityEvent(id,action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS ? AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED : AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
                        return true;
                    }
                    return false;
                }
            };
        }
    }
}
