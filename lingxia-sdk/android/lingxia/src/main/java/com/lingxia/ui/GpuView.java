package com.lingxia.ui;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Rect;
import android.os.Bundle;
import android.util.Log;
import android.view.Choreographer;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import android.widget.FrameLayout;
import android.widget.TextView;
import java.util.concurrent.ExecutorService;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * GPU page content. Construct and control on the UI thread; share one executor per app session.
 * Lifecycle tickets, input, scroll and accessibility policy live in the shared Rust core; this
 * class converts Android events into core calls and executes the effects it returns.
 */
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
    private static native int[] nativeRender(long page, int width, int height, float scale, boolean rebuild, boolean caretVisible, String offsets, long surface, long time);
    private static native void nativeAnimationPark(long page, long time);
    private static native long nativeAnimationPresented(long page, long surface, long generation, long time);
    private static native String nativeSnapshot(long page);
    private static native String nativeInput(long page, String request);
    private static native boolean nativeClick(long page, long generation, long handle);
    private static native boolean nativeNumber(long page, long generation, long handle, double value);
    private static native boolean nativePlot(long page, long generation, String identity, int mode, float x, float y, int index);
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
    private static native long nativeTakeFrame(long page);
    private static native void nativeFrameRelease(long frame);
    private static native boolean nativePointerState(long page, String pressed, String hovered);
    // Shared lifecycle core.
    static native long nativeLifecycleCreate(boolean logic);
    static native void nativeLifecycleRelease(long lifecycle);
    static native long nativeLifecycle(long lifecycle, int op, long value);
    static native String nativeLifecycleFail(long lifecycle, String error);
    // Shared interaction core; UI thread only.
    static native long nativeInteractionCreate();
    static native void nativeInteractionRelease(long interaction);
    static native String nativePresented(long interaction, long frame, long revision, String offsets, long page);
    static native String nativePointer(long interaction, int phase, float x, float y, float slop, float vx, float vy, float minFling, long page);
    static native String nativeSignal(long interaction, int op, long page);
    static native String nativeKey(long interaction, int key, boolean shift, boolean repeat, long page);
    static native String nativeQuery(long interaction, int op, long value);
    static native boolean nativeScrollBy(long interaction, String region, float delta);
    static native String nativeExplore(long interaction, boolean active, float x, float y);
    static native String nativeAccessibility(long interaction, int id, int action, double number, String argument, long page);

    // Lifecycle opcodes.
    private static final int LOAD = 0, LOADED = 1, PARK = 2, DISPOSE = 3, CONNECT = 4, BRIDGE_READY = 5, VISIBLE = 6,
        INVALIDATE_SURFACE = 7, PAGE = 8, SURFACE = 9, CONNECTION = 10, CURRENT_PAGE = 11, CURRENT_SURFACE = 12,
        CURRENT_CONNECTION = 13, FRAME = 14, ACCEPTS_INPUT = 15, SESSION_FAILED = 16, BRIDGE_IS_READY = 17;
    // Interaction signals and queries.
    private static final int LONG_PRESS = 0, CANCEL = 1, RESET = 2, FOCUS_FORWARD = 3, FOCUS_BACKWARD = 4, FOCUS_LEFT = 5,
        INERT = 6, ACTIVE = 7;
    private static final int SCROLL_STATE = 0, TAKE_DISPATCH = 1, TAKE_NUMBER = 2, NUMBER_DONE = 3, SEMANTIC_ROOTS = 4,
        SEMANTIC_NODE = 5, CAN_SCROLL_VERTICALLY = 6;

    public interface PullToRefresh { boolean onTouch(MotionEvent event); }
    private PullToRefresh pullToRefresh;
    public void setPullToRefresh(PullToRefresh callback) { pullToRefresh = callback; }
    public void clearPullToRefresh(PullToRefresh callback) { if (pullToRefresh == callback) pullToRefresh = null; }
    @Override public boolean canScrollVertically(int direction) {
        return "true".equals(query(CAN_SCROLL_VERTICALLY, direction));
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
    // Released after the last worker task; zero afterwards.
    private volatile long lifecycle;
    private long interaction;
    private TextView bootstrapMessage;
    private int bootstrapSurfaceImportance;
    private final Listener listener;
    private String source;
    // Written only on the worker; the lifecycle decides whether its results still apply.
    private long nativePage;
    private final UiSurface surface;
    private final GpuIme ime;
    private final View feedback;
    private boolean active, destroyed, busy, pending, numberBusy;
    private final GpuAnimationScheduler animationScheduler = new GpuAnimationScheduler(new GpuAnimationScheduler.Host() {
        public long now() { return android.os.SystemClock.uptimeMillis(); }
        public void post(Runnable callback, long delayMillis) { postOnAnimationDelayed(callback, delayMillis); }
        public void remove(Runnable callback) { removeCallbacks(callback); }
    }, () -> scheduleFrame());
    private boolean hasSurfaceFrame;
    private Bitmap bitmap;
    // The presented frame as JSON, for the input method and its chrome.
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
    private final GpuImages images = new GpuImages();
    private GpuTextMenu textMenu;
    private GpuSelectionHandles selectionHandles;
    private android.content.res.Configuration hostConfiguration;
    private org.json.JSONArray textScale;
    private float textScalePreference = Float.NaN, textScaleDensity = Float.NaN;
    private final GpuEnvironment hostEnvironment = new GpuEnvironment();
    private final boolean logic;

    public GpuView(Context context, String source, ExecutorService worker, Listener listener) {
        this(context, source, worker, listener, false);
    }
    public GpuView(Context context, String source, ExecutorService worker, Listener listener, boolean logic) {
        super(context);
        hostConfiguration = new android.content.res.Configuration(context.getResources().getConfiguration());
        this.logic = logic;
        this.worker = worker; this.listener = listener; this.source = source;
        lifecycle = nativeLifecycleCreate(logic);
        interaction = nativeInteractionCreate();
        // SurfaceView overlays can be hidden by its surface hole on physical devices.
        feedback = new View(context);
        feedback.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        surface = new UiSurface();
        ime = new GpuIme(surface, new GpuIme.Host() {
            @Override public void showTextMenu() { if (textMenu != null) textMenu.show(); }
            @Override public void hideTextMenu() { if (textMenu != null) textMenu.dismiss(); if (selectionHandles != null) selectionHandles.update(); }
            @Override public void editingChanged() {
                if (textMenu != null) textMenu.update();
                if (selectionHandles != null) {
                    selectionHandles.setBounds(0, 0, feedback.getWidth(), feedback.getHeight());
                    selectionHandles.update();
                }
            }
            @Override public void request(JSONObject request, java.util.function.Consumer<JSONObject> reply) {
                final long page = page();
                worker.execute(() -> {
                    if (!currentPage(page)) return;
                    JSONObject result = null;
                    try {
                        if (nativePage != 0) {
                            Object value = new org.json.JSONTokener(nativeInput(nativePage, request.toString())).nextValue();
                            if (value instanceof JSONObject) result = (JSONObject)value;
                        }
                    } catch (Throwable error) {
                        // Reject later queued edits against the failed mirror before they run.
                        if ("edit".equals(request.optString("op"))) {
                            try { nativeInput(nativePage, new JSONObject().put("op", "blur").put("identity", request.optString("identity")).put("token", request.optLong("token")).toString()); }
                            catch (Throwable ignored) { }
                        }
                        postError(error, page);
                    } finally {
                        // Draining actions invalidates View. Read-only hit queries must retain
                        // their frame generation until the following focus request is queued.
                        if (logic && nativePage != 0 && "edit".equals(request.optString("op"))
                                && (result == null || result.optBoolean("needs_view_render"))) {
                            try { publishBridgeUpdate(connection(), nativeDrainActions(nativePage)); }
                            catch (Throwable error) { postError(error, page); }
                        }
                    }
                    final JSONObject response = result;
                    main.post(() -> { if (!destroyed && currentPage(page)) reply.accept(response); });
                });
            }
            @Override public void render(boolean rebuild) {
                if (destroyed) return;
                viewDirty |= rebuild; scheduleFrame();
            }
        });
        textMenu = new GpuTextMenu(surface, ime, () -> {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            for (int i = 0; nodes != null && i < nodes.length(); i++) {
                JSONObject node = nodes.optJSONObject(i), input = node == null ? null : node.optJSONObject("input");
                if (input != null && input.optLong("edit_token") > 0) return surface.bounds(node);
            }
            return new Rect();
        });
        selectionHandles = new GpuSelectionHandles(ime, new GpuSelectionHandles.Host() {
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

    // ---- lifecycle tickets ----------------------------------------------------

    private long lc(int op, long value) {
        long handle = lifecycle;
        return handle == 0 ? 0 : nativeLifecycle(handle, op, value);
    }
    // Interaction calls are UI-thread only and inert after disposal.
    private String signal(int op) { return signal(op, page()); }
    private String signal(int op, long page) { return destroyed ? "[]" : nativeSignal(interaction, op, page); }
    private String query(int op, long value) { return destroyed ? "null" : nativeQuery(interaction, op, value); }
    private boolean scrollRegion(String region, float delta) { return !destroyed && nativeScrollBy(interaction, region, delta); }
    private static final String UNHANDLED = "{\"handled\":false,\"effects\":[]}";
    private String presented(long frame, long revision, String offsets, long page) {
        if (destroyed) { nativeFrameRelease(frame); return "{\"effects\":[]}"; }
        return nativePresented(interaction, frame, revision, offsets, page);
    }
    private String pointer(int phase, float x, float y, float slop, float vx, float vy, float minFling, long page) {
        return destroyed ? UNHANDLED : nativePointer(interaction, phase, x, y, slop, vx, vy, minFling, page);
    }
    private String keyEvent(int key, boolean shift, boolean repeat, long page) {
        return destroyed ? UNHANDLED : nativeKey(interaction, key, shift, repeat, page);
    }
    private String explore(boolean exploring, float x, float y) {
        return destroyed ? UNHANDLED : nativeExplore(interaction, exploring, x, y);
    }
    private String accessibilityAction(int id, int action, double number, String argument, long page) {
        return destroyed ? UNHANDLED : nativeAccessibility(interaction, id, action, number, argument, page);
    }
    private long page() { return lc(PAGE, 0); }
    private boolean bridgeReady() { return lc(BRIDGE_IS_READY, 0) != 0; }
    private boolean currentPage(long page) { return lc(CURRENT_PAGE, page) != 0; }
    private boolean currentSurface(long surface) { return lc(CURRENT_SURFACE, surface) != 0; }
    private boolean currentConnection(long connection) { return lc(CURRENT_CONNECTION, connection) != 0; }
    private long connection() { return lc(CONNECTION, 0); }
    private void invalidateSurface() { lc(INVALIDATE_SURFACE, 0); }

    public void reload() {
        reload(source);
    }
    public void reload(String source) {
        if (destroyed) return;
        animationScheduler.cancel();
        clearBootstrap();
        recovery.clear();
        this.source = source;
        final String module = source;
        final long page = lc(LOAD, 0);
        ime.reset();
        snapshot = null;
        apply(signal(RESET, page));
        signal(ACTIVE, page);
        images.clear(); viewDirty = true;
        if (surface.getParent() != this) {
            removeAllViews();
            addView(surface, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            addView(feedback, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        }
        worker.execute(() -> {
            try {
                if (nativePage != 0) nativeRelease(nativePage);
                nativePage = 0;
                nativePage = nativeLoad(module);
                main.post(() -> {
                    if (!destroyed && lc(LOADED, page) != 0) {
                        if (listener != null) listener.onLoaded();
                        requestFrame();
                    }
                });
            } catch (Throwable error) { postError(error, page); }
        });
    }

    private void postError(Throwable error, long page) {
        main.post(() -> { if (!destroyed && currentPage(page)) showError(error); });
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
        JSONObject recovered;
        try { recovered = new JSONObject(nativeLifecycleFail(lifecycle, String.valueOf(error))); }
        catch (org.json.JSONException impossible) { throw new IllegalStateException(impossible); }
        GpuRecovery.Kind kind = GpuRecovery.Kind.valueOf(recovered.optString("kind").toUpperCase(java.util.Locale.ROOT));
        String retry = recovered.optString("retry");
        if (kind == GpuRecovery.Kind.SESSION) {
            apply(signal(INERT));
            ime.reset();
        }
        final long connection = connection();
        Runnable action = null;
        switch (retry) {
            case "Redraw": action = () -> { if (!destroyed) { viewDirty = true; scheduleFrame(); } }; break;
            case "Reload": action = () -> { if (!destroyed) reload(); }; break;
            case "Restart": action = () -> {
                if (!destroyed && (listener == null || !listener.onRestart(connection))) recovery.show(snapshot != null, kind, null);
            }; break;
            default: break;
        }
        recovery.show(snapshot != null, kind, action);
    }

    public void resume() { active = true; updateSurfaceVisibility(); requestFrame(); pumpImages(); }
    @Override protected void onVisibilityChanged(View changedView, int visibility) {
        super.onVisibilityChanged(changedView, visibility);
        updateSurfaceVisibility();
        if (active && isShown()) scheduleFrame();
    }
    @Override protected void onWindowVisibilityChanged(int visibility) {
        super.onWindowVisibilityChanged(visibility);
        syncVisibility();
        if (visibility != View.VISIBLE) parkAnimations();
        else if (active && isShown()) scheduleFrame();
    }
    private void syncVisibility() {
        lc(VISIBLE, active && isShown() && getWindowVisibility() == View.VISIBLE && !destroyed ? 1 : 0);
    }
    private void updateSurfaceVisibility() {
        if (surface == null) return;
        syncVisibility();
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
            if (!focused) { surface.cancelRefreshGesture(); apply(signal(CANCEL)); }
            updateCaretActivity();
        }
    }
    public long getConnectionGeneration() { return connection(); }
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
        final long captured = lc(SURFACE, 0);
        try {
            android.view.PixelCopy.request(surface, capture, result -> {
                if (result == android.view.PixelCopy.SUCCESS && !destroyed && currentSurface(captured)) {
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
    public void pause() {
        surface.cancelRefreshGesture(); finishSurfaceRedraws();
        active = false; updateSurfaceVisibility(); ime.clear(true);
        apply(signal(CANCEL));
    }
    public void park() {
        if (destroyed) return;
        clearBootstrap();
        recovery.clear();
        final long page = lc(PARK, 0);
        pause(); snapshot = null; apply(signal(RESET, page)); images.clear(); viewDirty = true;
        worker.execute(() -> {
            try { if (nativePage != 0) nativeReset(nativePage); }
            catch (Throwable error) { postError(error, page); }
        });
    }

    public void connect(long generation, String nonce) {
        if (destroyed || lc(CONNECT, generation) == 0) return;
        clearBootstrap();
        recovery.clear();
        bootstrap.begin(() -> {
            if (destroyed || !currentConnection(generation) || lc(BRIDGE_IS_READY, 0) != 0) return;
            bootstrapMessage = new TextView(getContext());
            bootstrapMessage.setText("This page is taking longer to load. You can go back and try again.");
            bootstrapMessage.setTextColor(android.graphics.Color.BLACK);
            bootstrapMessage.setBackgroundColor(android.graphics.Color.WHITE);
            bootstrapMessage.setPadding(32, 48, 32, 32);
            bootstrapMessage.setClickable(true);
            bootstrapSurfaceImportance = surface.getImportantForAccessibility();
            surface.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
            addView(bootstrapMessage, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            Log.e("LingXia.UI", "UI_BOOTSTRAP_TIMEOUT: bridge ready and initial state not received within 5 seconds");
        });
        worker.execute(() -> {
            if (!currentConnection(generation)) return;
            try {
                String hello = nativeBeginBridge(nativePage, nonce);
                main.post(() -> {
                    if (!destroyed && currentConnection(generation) && listener != null) listener.onMessage(generation, hello);
                });
            } catch (Throwable error) { main.post(() -> { if (currentConnection(generation)) showError(error); }); }
        });
    }

    public void receive(long generation, String frame) {
        if (destroyed || !currentConnection(generation)) return;
        worker.execute(() -> {
            if (!currentConnection(generation)) return;
            try { publishBridgeUpdate(generation, nativeReceiveBridge(nativePage, frame)); }
            catch (Throwable error) { main.post(() -> { if (currentConnection(generation)) showError(error); }); }
        });
    }

    public void requestLeave(long generation, String reason) {
        if (destroyed || !currentConnection(generation)) return;
        worker.execute(() -> {
            if (!currentConnection(generation) || nativePage == 0) return;
            try { publishBridgeUpdate(generation, nativeRequestLeave(nativePage, reason)); }
            catch (Throwable error) { main.post(() -> { if (!destroyed && currentConnection(generation)) showError(error); }); }
        });
    }

    private void publishBridgeUpdate(long generation, String encoded) throws org.json.JSONException {
        JSONObject update = new JSONObject(encoded);
        JSONArray outbound = update.getJSONArray("outbound"), errors = update.getJSONArray("errors");
        for (int i = 0; i < errors.length(); i++) Log.e("LingXia.UI", errors.getString(i));
        final boolean canRender = update.getBoolean("ready"), changed = update.getBoolean("changed");
        main.post(() -> {
            if (destroyed || !currentConnection(generation)) return;
            try {
                // State acknowledgements leave the View before any rendering attempt.
                for (int i = 0; i < outbound.length(); i++) if (listener != null) listener.onMessage(generation, outbound.get(i).toString());
                lc(BRIDGE_READY, canRender ? 1 : 0);
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
        surface.stopFling();
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
        apply(signal(CANCEL));
        lc(DISPOSE, 0);
        destroyed = true;
        images.close(); ime.clear(false); surface.stopFling();
        if (gpuSurface != 0) { nativeDetach(gpuSurface); gpuSurface = 0; }
        nativeInteractionRelease(interaction);
        interaction = 0;
        // Worker tasks queued before disposal still read the lifecycle; release it after them.
        worker.execute(() -> {
            if (nativePage != 0) { nativeRelease(nativePage); nativePage = 0; }
            long handle = lifecycle;
            lifecycle = 0;
            nativeLifecycleRelease(handle);
        });
        if (bitmap != null) { bitmap.recycle(); bitmap = null; }
    }
    @Override public void surfaceCreated(SurfaceHolder holder) { updateCaretActivity(); }
    @Override public void surfaceDestroyed(SurfaceHolder holder) {
        surface.cancelRefreshGesture();
        parkAnimations();
        hasSurfaceFrame = false;
        invalidateSurface();
        finishSurfaceRedraws();
        ime.setCaretActive(false);
        apply(signal(CANCEL));
        gpuAttached = false;
        if (gpuSurface != 0) nativeSuspend(gpuSurface);
    }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        hasSurfaceFrame = false;
        invalidateSurface();
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
        final long requested = lc(SURFACE, 0);
        final int width = surface.getWidth(), height = surface.getHeight();
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
                if (destroyed || !currentSurface(requested)) {
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
    private void acknowledgeAnimation(long requested, long generation) {
        final long time = android.os.SystemClock.uptimeMillis();
        worker.execute(() -> {
            try {
                if (nativePage == 0) return;
                final long delay = nativeAnimationPresented(nativePage, requested, generation, time);
                main.post(() -> {
                    if (delay < 0 || destroyed || !currentSurface(requested) || snapshot == null
                        || snapshot.optLong("generation") != generation || busy || pending) return;
                    animationScheduler.schedule(delay, time);
                });
            } catch (Throwable error) { Log.e("LingXia.UI", "Animation presentation acknowledgement failed", error); }
        });
    }
    private void scheduleFrame() {
        animationScheduler.cancel();
        pending = true;
        syncVisibility();
        if (destroyed || lc(FRAME, 0) == 0 || !surface.getHolder().getSurface().isValid()) return;
        final int width = surface.getWidth(), height = surface.getHeight();
        if (width <= 0 || height <= 0) return;
        final float scale = getResources().getDisplayMetrics().density;
        final String environment = measureEnvironment(scale);
        if (!environment.equals(frameEnvironment)) { frameEnvironment = environment; invalidateSurface(); viewDirty = true; }
        if (busy || attaching || numberBusy) return;
        final long requested = lc(FRAME, 0);
        if (requested == 0) return;
        final long page = page();
        final long targetSurface = gpuSurface;
        final boolean rebuild = viewDirty || snapshot == null;
        final JSONObject scrollState = json(query(SCROLL_STATE, 0));
        final long scrollRevision = scrollState.optLong("revision");
        final String offsets = scrollState.optJSONObject("offsets").toString();
        final GpuIme.Ticket inputTicket = ime.capture();
        final boolean caretVisible = ime.caretVisible();
        viewDirty = false;
        pending = false; busy = true;
        final long queuedAt = System.nanoTime();
        final long animationTime = android.os.SystemClock.uptimeMillis();
        worker.execute(() -> {
            long frame = 0;
            try {
                long started = System.nanoTime();
                final long workerQueueUs = (started - queuedAt) / 1000;
                nativeEnvironment(nativePage, environment);
                Bitmap rendered = null;
                if (targetSurface != 0) nativePrepareGpu(nativePage, targetSurface, width, height, scale, rebuild, caretVisible, offsets, requested, animationTime);
                else rendered = Bitmap.createBitmap(nativeRender(nativePage, width, height, scale, rebuild, caretVisible, offsets, requested, animationTime), width, height, Bitmap.Config.ARGB_8888);
                frame = nativeTakeFrame(nativePage);
                if (logic) {
                    JSONObject committed = new JSONObject(nativeDrainActions(nativePage));
                    committed.put("changed", committed.optBoolean("localChanged"));
                    publishBridgeUpdate(connection(), committed.toString());
                }
                final Bitmap next = rendered;
                final long presentedFrame = frame;
                frame = 0;
                final String metadata = nativeSnapshot(nativePage);
                final JSONObject nextSnapshot = new JSONObject(metadata);
                final long elapsed = (System.nanoTime() - started) / 1000000;
                final long preparedAt = System.nanoTime();
                main.post(() -> Choreographer.getInstance().postFrameCallback(time -> {
                    final long presentStarted = System.nanoTime();
                    busy = false;
                    boolean presented = false;
                    if (!destroyed && currentSurface(requested) && surface.getHolder().getSurface().isValid() && surface.present(next, nextSnapshot)) {
                        presented = true;
                        hasSurfaceFrame = true;
                        if (bitmap != null) bitmap.recycle();
                        bitmap = next;
                        snapshot = nextSnapshot; frameScale = scale;
                        // Repainting an old frame (caret/scroll) is not recovery.
                        if (rebuild) recovery.clear();
                        JSONObject outcome = json(presented(presentedFrame, scrollRevision, offsets, page));
                        acknowledgeAnimation(requested, nextSnapshot.optLong("generation"));
                        ime.presented(nextSnapshot, inputTicket);
                        updateCaretActivity();
                        apply(outcome.optJSONArray("effects"));
                        dispatchReach(outcome.optJSONObject("reach"));
                        updateImages(nextSnapshot.optJSONArray("images"));
                        if (listener != null) listener.onFrame(metadata);
                        Log.i("LingXia.UI", "present generation=" + snapshot.optLong("generation") + " surface=" + targetSurface + " size=" + width + "x" + height + " workerMs=" + elapsed + " workerQueueUs=" + workerQueueUs + " uiWaitUs=" + ((presentStarted - preparedAt) / 1000) + " uiUs=" + ((System.nanoTime() - presentStarted) / 1000) + " rebuild=" + rebuild + " caretVisible=" + caretVisible + " nodes=" + snapshot.optJSONArray("nodes").length() + " images=" + snapshot.optInt("loadedImages") + " metrics=" + snapshot.optJSONObject("metrics"));
                    } else {
                        nativeFrameRelease(presentedFrame);
                        if (next != null) next.recycle();
                    }
                    if (presented) finishPresentation(true);
                    if (pending && !destroyed) scheduleFrame();
                }));
            } catch (Throwable error) {
                if (frame != 0) nativeFrameRelease(frame);
                main.post(() -> {
                    busy = false; viewDirty = true;
                    if (!destroyed && currentPage(page)) {
                        apply(signal(CANCEL, page));
                        if (currentSurface(requested)) showError(error);
                    }
                    if (pending && !destroyed) scheduleFrame();
                });
            }
        });
    }

    // ---- effects --------------------------------------------------------------

    private static JSONObject json(String encoded) {
        try { return new JSONObject(encoded); }
        catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native reply", error); }
    }
    private static JSONArray array(String encoded) {
        try { return new JSONArray(encoded); }
        catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native reply", error); }
    }

    private void apply(String encoded) { apply(array(encoded)); }
    /** Execute core effects with Android mechanics. */
    private void apply(JSONArray effects) {
        for (int i = 0; effects != null && i < effects.length(); i++) {
            JSONObject effect = effects.optJSONObject(i);
            switch (effect.optString("kind")) {
                case "press":
                    if (effect.optBoolean("styled")) setPressedStyle(effect.optString("identity"));
                    else surface.showPress(effect.optInt("node"));
                    break;
                case "clear-press": surface.clearPress(); setPressedStyle(null); break;
                case "focus-ring": surface.showFocusRing(effect.optInt("node")); break;
                case "clear-focus-ring": surface.clearFocusRing(); break;
                case "redraw": scheduleFrame(); break;
                case "claim": if (surface.getParent() != null) surface.getParent().requestDisallowInterceptTouchEvent(effect.optBoolean("exclusive")); break;
                case "arm-long-press": surface.armLongPress(); break;
                case "cancel-long-press": surface.disarmLongPress(); break;
                case "fling": surface.fling(effect.optString("region"), effect.optBoolean("horizontal"), (float)effect.optDouble("velocity")); break;
                case "stop-fling": surface.stopFling(); break;
                case "dispatch": drainDispatch(); break;
                case "number": drainNumber(); break;
                case "clicked": surface.performClick(); break;
                case "haptic": surface.performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS); break;
                case "edit-text": {
                    JSONObject node = surface.node(effect.optInt("node"));
                    if (node != null && snapshot != null) {
                        String text = effect.isNull("text") ? null : effect.optString("text");
                        ime.accessibilityEdit(node, snapshot.optLong("generation"), text, effect.optInt("start"), effect.optInt("end"));
                    }
                    break;
                }
                case "accessibility": surface.announce(effect); break;
                default: break;
            }
        }
    }

    private String styledPress;
    /** `active:` styles repaint in the session; only the transition is forwarded. */
    private void setPressedStyle(String identity) {
        if (java.util.Objects.equals(identity, styledPress)) return;
        styledPress = identity;
        final long page = page();
        worker.execute(() -> {
            try {
                if (currentPage(page) && nativePage != 0 && nativePointerState(nativePage, identity, null))
                    main.post(() -> { if (!destroyed && currentPage(page)) scheduleFrame(); });
            } catch (Throwable error) { postError(error, page); }
        });
    }

    /** Run queued activations against the frame the core resolved them on. */
    private void drainDispatch() {
        if (busy || destroyed) return;
        JSONArray dispatches = array(query(TAKE_DISPATCH, 0));
        for (int i = 0; i < dispatches.length(); i++) {
            JSONObject dispatch = dispatches.optJSONObject(i);
            final long commit = dispatch.optLong("commit");
            final long page = page();
            switch (dispatch.optString("kind")) {
                case "editor": {
                    JSONObject node = surface.node(dispatch.optInt("node"));
                    JSONArray point = dispatch.optJSONArray("point");
                    if (node == null) break;
                    if (point == null) ime.focus(node, commit);
                    else ime.touch(node, commit, (float)point.optDouble(0), (float)point.optDouble(1), dispatch.optBoolean("word"));
                    break;
                }
                case "click": {
                    if (ime.isActive()) ime.clear(true);
                    final long handle = dispatch.optLong("handle");
                    runOnWorker(page, () -> nativeClick(nativePage, commit, handle), true);
                    break;
                }
                case "plot": {
                    if (ime.isActive()) ime.clear(true);
                    final String identity = dispatch.optString("identity");
                    final int mode = dispatch.optInt("mode"), index = dispatch.optInt("index");
                    final JSONArray point = dispatch.optJSONArray("point");
                    runOnWorker(page, () -> nativePlot(nativePage, commit, identity, mode, (float)point.optDouble(0), (float)point.optDouble(1), index), true);
                    break;
                }
                case "dismiss-plots": {
                    final JSONArray identities = dispatch.optJSONArray("identities");
                    runOnWorker(page, () -> {
                        boolean changed = false;
                        for (int j = 0; j < identities.length(); j++) changed |= nativePlot(nativePage, commit, identities.optString(j), 2, 0, 0, -1);
                        return changed;
                    }, false);
                    break;
                }
                default: break;
            }
        }
    }

    private interface WorkerCall { boolean run() throws Exception; }
    private void runOnWorker(long page, WorkerCall call, boolean announce) {
        worker.execute(() -> {
            if (!currentPage(page) || nativePage == 0) return;
            try {
                if (call.run()) {
                    if (logic) publishBridgeUpdate(connection(), nativeDrainActions(nativePage));
                    main.post(() -> {
                        if (destroyed || !currentPage(page)) return;
                        if (announce) surface.sendAccessibilityEvent(AccessibilityEvent.TYPE_VIEW_CLICKED);
                        requestFrame();
                    });
                }
            } catch (Throwable error) { postError(error, page); }
        });
    }

    private void dispatchReach(JSONObject reach) {
        if (reach == null) return;
        final long page = page(), commit = reach.optLong("commit");
        final JSONArray handles = reach.optJSONArray("handles");
        final boolean more = reach.optBoolean("more");
        // Queue before any next preparation, so generation validation sees the presented frame.
        worker.execute(() -> {
            if (!currentPage(page)) return;
            try {
                boolean changed = false;
                for (int i = 0; handles != null && i < handles.length(); i++) changed |= nativeClick(nativePage, commit, handles.optLong(i));
                if (logic && changed) publishBridgeUpdate(connection(), nativeDrainActions(nativePage));
                if (changed || more) main.post(() -> { if (!destroyed && currentPage(page)) requestFrame(); });
            } catch (Throwable error) { postError(error, page); }
        });
    }

    private void drainNumber() {
        if (busy || numberBusy || destroyed) return;
        String encoded = query(TAKE_NUMBER, 0);
        if ("null".equals(encoded)) return;
        JSONObject proposal = json(encoded);
        final long commit = proposal.optLong("commit"), handle = proposal.optLong("handle"), page = proposal.optLong("page");
        final double value = proposal.optDouble("value");
        numberBusy = true;
        worker.execute(() -> {
            boolean changed = false; Throwable failure = null;
            try {
                if (currentPage(page) && nativePage != 0) {
                    changed = nativeNumber(nativePage, commit, handle, value);
                    if (changed && logic) publishBridgeUpdate(connection(), nativeDrainActions(nativePage));
                }
            } catch (Throwable error) { failure = error; }
            final boolean applied = changed; final Throwable error = failure;
            main.post(() -> {
                numberBusy = false;
                if (destroyed) return;
                query(NUMBER_DONE, 0);
                if (error != null) { apply(signal(CANCEL)); if (currentPage(page)) showError(error); return; }
                if (applied) requestFrame(); else if (pending) scheduleFrame(); else drainNumber();
            });
        });
    }

    private void updateImages(JSONArray sources) {
        images.update(sources == null ? new JSONArray() : sources); pumpImages(); flushImageEvents();
    }
    private void flushImageEvents() {
        if (destroyed || snapshot == null) return;
        final long page = page(), generation = snapshot.optLong("generation");
        worker.execute(() -> {
            try {
                if (!currentPage(page) || nativePage == 0) return;
                if (nativeImageEvents(nativePage, generation)) {
                    if (logic) publishBridgeUpdate(connection(), nativeDrainActions(nativePage));
                    main.post(() -> { if (!destroyed && currentPage(page)) requestFrame(); });
                }
            } catch (Throwable error) { postError(error, page); }
        });
    }
    private void pumpImages() {
        if (destroyed || !active || listener == null) return;
        for (GpuImages.Request request : images.start()) {
            final long page = page();
            Runnable cancel = listener.onImageRequest(request.source, bytes -> main.post(() -> {
                if (destroyed || !images.current(request)) {
                    images.complete(request); pumpImages(); return;
                }
                worker.execute(() -> {
                    boolean changed = false;
                    boolean current = currentPage(page) && nativePage != 0;
                    try { if (current && bytes != null) changed = images.apply(nativePage, request, bytes); }
                    catch (Throwable error) { Log.w("LingXia.UI", "Image failed: " + request.source, error); }
                    if (current) images.status(nativePage, request, changed);
                    final boolean updated = changed;
                    main.post(() -> {
                        boolean live = images.complete(request);
                        if (!destroyed && live && updated) scheduleFrame();
                        if (!destroyed && live) flushImageEvents();
                        pumpImages();
                    });
                });
            }));
            images.track(request, cancel);
        }
    }

    private static final int[] KEYS = {KeyEvent.KEYCODE_TAB, KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN,
        KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MOVE_HOME, KeyEvent.KEYCODE_MOVE_END};
    private static int key(int code) {
        for (int i = 0; i < KEYS.length; i++) if (KEYS[i] == code) return i;
        return code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_SPACE || code == KeyEvent.KEYCODE_DPAD_CENTER ? 7 : -1;
    }

    private final class UiSurface extends SurfaceView {
        private final GpuHighlight.Highlight pressedOverlay = new GpuHighlight.Highlight();
        private final GpuHighlight.Highlight focusOverlay = new GpuHighlight.Highlight();
        private final int slop;
        private android.view.VelocityTracker velocity;
        private final android.widget.OverScroller scroller = new android.widget.OverScroller(getContext());
        private String flingRegion;
        private int flingPosition;
        private boolean flingHorizontal;
        private boolean refreshingGesture;
        private final Runnable longPress = () -> apply(signal(LONG_PRESS));
        private final Choreographer.FrameCallback flingFrame = new Choreographer.FrameCallback() {
            @Override public void doFrame(long time) {
                if (!active || destroyed || flingRegion == null || !scroller.computeScrollOffset()) return;
                int position = flingHorizontal ? scroller.getCurrX() : scroller.getCurrY();
                boolean changed = scrollRegion(flingRegion, position - flingPosition);
                flingPosition = position;
                if (!changed) { scroller.abortAnimation(); flingRegion = null; return; }
                scheduleFrame();
                Choreographer.getInstance().postFrameCallback(this);
            }
        };

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
            return GpuAccessibility.pixels(b, frameScale);
        }
        void showPress(int index) {
            JSONObject node = node(index);
            if (node == null) return;
            pressedOverlay.setNode(node, frameScale);
            feedback.getOverlay().remove(pressedOverlay);
            feedback.getOverlay().add(pressedOverlay);
        }
        void clearPress() { feedback.getOverlay().remove(pressedOverlay); }
        void showFocusRing(int index) {
            JSONObject node = node(index);
            if (node == null) return;
            focusOverlay.setNode(node, frameScale);
            focusOverlay.setStroke(Math.max(1, (int)(2 * frameScale)), Color.rgb(23, 32, 51));
            feedback.getOverlay().remove(focusOverlay);
            feedback.getOverlay().add(focusOverlay);
        }
        void clearFocusRing() { feedback.getOverlay().remove(focusOverlay); }
        void clearFeedback() { clearPress(); clearFocusRing(); }
        void armLongPress() { main.removeCallbacks(longPress); main.postDelayed(longPress, ViewConfiguration.getLongPressTimeout()); }
        void disarmLongPress() { main.removeCallbacks(longPress); }
        void fling(String region, boolean horizontal, float speed) {
            flingRegion = region;
            flingHorizontal = horizontal;
            flingPosition = 0;
            if (flingHorizontal) scroller.fling(0, 0, (int)speed, 0, -100000000, 100000000, 0, 0);
            else scroller.fling(0, 0, 0, (int)speed, 0, 0, -100000000, 100000000);
            Choreographer.getInstance().postFrameCallback(flingFrame);
        }
        void stopFling() {
            scroller.abortAnimation();
            Choreographer.getInstance().removeFrameCallback(flingFrame);
            flingRegion = null;
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
        boolean present(Bitmap candidate, JSONObject frame) {
            if (!active || !getHolder().getSurface().isValid()) return false;
            if (gpuSurface != 0) {
                try {
                    long started = System.nanoTime();
                    if (!nativePresentGpu(gpuSurface, frame.optLong("generation"))) {
                        animationScheduler.schedule(0, android.os.SystemClock.uptimeMillis());
                        return false;
                    }
                    Log.i("LingXia.UI", "gpuSubmitMs=" + ((System.nanoTime() - started) / 1000000));
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
            if (lc(SESSION_FAILED, 0) != 0) return false;
            if (selectionHandles.touch(event)) {
                apply(signal(CANCEL));
                getParent().requestDisallowInterceptTouchEvent(selectionHandles.dragging());
                return true;
            }
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) refreshingGesture = false;
            boolean refreshConsumed = pullToRefresh != null && pullToRefresh.onTouch(event);
            if (refreshConsumed || refreshingGesture) {
                if (!refreshingGesture) {
                    apply(signal(CANCEL));
                    refreshingGesture = true;
                }
                if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) refreshingGesture = false;
                return true;
            }
            int phase;
            switch (action) {
                case MotionEvent.ACTION_DOWN: phase = 0; textMenu.dismiss(); break;
                case MotionEvent.ACTION_MOVE: phase = 1; break;
                case MotionEvent.ACTION_UP: phase = 2; break;
                case MotionEvent.ACTION_CANCEL: case MotionEvent.ACTION_POINTER_DOWN: phase = 3; break;
                default: return true;
            }
            if (phase == 0) { if (velocity != null) velocity.recycle(); velocity = android.view.VelocityTracker.obtain(); }
            if (velocity != null) velocity.addMovement(event);
            float vx = 0, vy = 0;
            ViewConfiguration configuration = ViewConfiguration.get(getContext());
            if (phase == 2 && velocity != null) {
                velocity.computeCurrentVelocity(1000, configuration.getScaledMaximumFlingVelocity());
                vx = velocity.getXVelocity(); vy = velocity.getYVelocity();
            }
            JSONObject outcome = json(pointer(phase, event.getX(), event.getY(), slop, vx, vy,
                configuration.getScaledMinimumFlingVelocity(), page()));
            apply(outcome.optJSONArray("effects"));
            if (phase >= 2 && velocity != null) { velocity.recycle(); velocity = null; }
            if (phase == 0 && !outcome.optBoolean("handled")) return pullToRefresh != null;
            return true;
        }
        @Override public boolean performClick() { super.performClick(); return true; }
        @Override public boolean onCheckIsTextEditor() { return ime != null && ime.hasFocus(); }
        @Override public android.view.inputmethod.InputConnection onCreateInputConnection(android.view.inputmethod.EditorInfo info) {
            return ime == null ? null : ime.create(info);
        }
        @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
            if (lc(SESSION_FAILED, 0) != 0) return super.onKeyDown(keyCode, event);
            if (ime.hasFocus() && keyCode != KeyEvent.KEYCODE_TAB) return ime.key(event) || super.onKeyDown(keyCode, event);
            if (ime.hasFocus()) ime.clear(true);
            int key = key(keyCode);
            if (key >= 0) {
                JSONObject outcome = json(keyEvent(key, event.isShiftPressed(), event.getRepeatCount() > 0, page()));
                apply(outcome.optJSONArray("effects"));
                if (outcome.optBoolean("handled")) return true;
            }
            return super.onKeyDown(keyCode, event);
        }
        @Override public boolean onKeyShortcut(int keyCode, KeyEvent event) {
            if (lc(SESSION_FAILED, 0) != 0) return super.onKeyShortcut(keyCode, event);
            return ime.hasFocus() ? ime.key(event) : super.onKeyShortcut(keyCode, event);
        }
        @Override protected void onFocusChanged(boolean focused, int direction, Rect previouslyFocusedRect) {
            super.onFocusChanged(focused, direction, previouslyFocusedRect);
            if (!focused) { apply(signal(FOCUS_LEFT)); if (ime != null) ime.clear(true); }
            else if (!isInTouchMode() && (ime == null || !ime.hasFocus()))
                apply(signal(direction == View.FOCUS_BACKWARD || direction == View.FOCUS_UP ? FOCUS_BACKWARD : FOCUS_FORWARD));
        }
        @Override public boolean dispatchHoverEvent(MotionEvent event) {
            android.view.accessibility.AccessibilityManager manager = (android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            if (!active || destroyed || manager == null || !manager.isEnabled() || !manager.isTouchExplorationEnabled()) {
                apply(json(explore(false, 0, 0)).optJSONArray("effects"));
                return super.dispatchHoverEvent(event);
            }
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_HOVER_ENTER:
                case MotionEvent.ACTION_HOVER_MOVE: {
                    JSONObject outcome = json(explore(true, event.getX(), event.getY()));
                    apply(outcome.optJSONArray("effects"));
                    return outcome.optBoolean("handled");
                }
                case MotionEvent.ACTION_HOVER_EXIT: {
                    JSONObject outcome = json(explore(false, 0, 0));
                    apply(outcome.optJSONArray("effects"));
                    return true;
                }
                default: return false;
            }
        }
        private boolean accessibilityEnabled() {
            android.view.accessibility.AccessibilityManager manager = (android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            return getParent() != null && manager != null && manager.isEnabled();
        }
        private AccessibilityEvent event(int id, int type) {
            AccessibilityEvent event = AccessibilityEvent.obtain(type);
            // Virtual descendants must be invalidated, not only the host node.
            if (type == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED) event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
            event.setSource(this, id); event.setPackageName(getContext().getPackageName());
            return event;
        }
        private void send(AccessibilityEvent event) { getParent().requestSendAccessibilityEvent(this, event); }
        /** Translate a core accessibility effect into Android events. */
        void announce(JSONObject effect) {
            if (!accessibilityEnabled()) return;
            String kind = effect.optString("event");
            int id = effect.optInt("id", View.NO_ID);
            switch (kind) {
                case "focused": send(event(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED)); break;
                case "focus-cleared": send(event(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED)); break;
                case "input-focused": send(event(id, AccessibilityEvent.TYPE_VIEW_FOCUSED)); break;
                case "hover-enter": send(event(id, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER)); break;
                case "hover-exit": send(event(id, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT)); break;
                case "change": {
                    JSONObject change = effect;
                    switch (change.optString("kind")) {
                        case "text-changed": {
                            AccessibilityEvent event = event(change.optInt("id"), AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED);
                            String before = change.optString("before"), after = change.optString("after");
                            event.setClassName("android.widget.EditText"); event.setPassword(change.optBoolean("secure"));
                            event.setBeforeText(before); event.getText().add(after); event.setFromIndex(0);
                            event.setRemovedCount(before.length()); event.setAddedCount(after.length());
                            send(event);
                            break;
                        }
                        case "selection-changed": {
                            AccessibilityEvent event = event(change.optInt("id"), AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
                            String text = change.optString("text");
                            event.setClassName("android.widget.EditText"); event.setPassword(change.optBoolean("secure"));
                            event.getText().add(text); event.setFromIndex(change.optInt("start")); event.setToIndex(change.optInt("end"));
                            event.setItemCount(text.length());
                            send(event);
                            break;
                        }
                        case "value-changed": send(event(change.optInt("id"), AccessibilityEvent.TYPE_VIEW_SELECTED)); break;
                        case "content-changed": send(event(View.NO_ID, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED)); break;
                        default: break;
                    }
                    break;
                }
                default: break;
            }
        }
        @Override public AccessibilityNodeProvider getAccessibilityNodeProvider() {
            return new AccessibilityNodeProvider() {
                private JSONObject roots() {
                    String encoded = query(SEMANTIC_ROOTS, 0);
                    return "null".equals(encoded) ? null : json(encoded);
                }
                private JSONObject semantic(int id) {
                    String encoded = query(SEMANTIC_NODE, id);
                    return "null".equals(encoded) ? null : json(encoded);
                }
                @Override public AccessibilityNodeInfo findFocus(int focus) {
                    if (!active || destroyed) return null;
                    JSONObject roots = roots();
                    if (roots == null) return null;
                    String key = focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY ? "accessibilityFocus"
                        : focus == AccessibilityNodeInfo.FOCUS_INPUT ? "inputFocus" : null;
                    if (key == null || roots.isNull(key)) return null;
                    return createAccessibilityNodeInfo(roots.optInt(key));
                }
                @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
                    if (destroyed) return null;
                    if (id == View.NO_ID) {
                        AccessibilityNodeInfo root = AccessibilityNodeInfo.obtain(UiSurface.this);
                        onInitializeAccessibilityNodeInfo(root);
                        JSONObject roots = active ? roots() : null;
                        JSONArray children = roots == null ? null : roots.optJSONArray("roots");
                        for (int i = 0; children != null && i < children.length(); i++) root.addChild(UiSurface.this, children.optInt(i));
                        return root;
                    }
                    JSONObject node = semantic(id);
                    if (node == null) return null;
                    JSONObject parent = node.isNull("parent") ? null : semantic(node.optInt("parent"));
                    JSONObject roots = roots();
                    boolean focused = roots != null && !roots.isNull("accessibilityFocus") && roots.optInt("accessibilityFocus") == id;
                    AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain();
                    info.setSource(UiSurface.this, id);
                    GpuAccessibility.populate(info, UiSurface.this, node, parent, frameScale, focused, getContext().getPackageName());
                    if (!active) info.setVisibleToUser(false);
                    return info;
                }
                @Override public boolean performAction(int id, int action, Bundle args) {
                    if (!active || destroyed) return false;
                    int shared = GpuAccessibility.action(action);
                    if (shared < 0) return false;
                    double number = 0; String argument = "";
                    if (shared == GpuAccessibility.PROGRESS) {
                        if (args == null || !args.containsKey(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE)) return false;
                        number = args.getFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, Float.NaN);
                    } else if (shared == GpuAccessibility.SET_TEXT) {
                        CharSequence value = args == null ? null : args.getCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE);
                        if (value != null && value.length() > 65536) return false;
                        argument = value == null ? "" : value.toString();
                    } else if (shared == GpuAccessibility.SET_SELECTION) {
                        number = args == null ? -1 : args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, -1);
                        argument = String.valueOf(args == null ? -1 : args.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, -1));
                    } else if (shared == GpuAccessibility.SELECT) {
                        number = action - GpuAccessibility.SELECT_ITEM;
                    }
                    JSONObject outcome = json(accessibilityAction(id, shared, number, argument, page()));
                    apply(outcome.optJSONArray("effects"));
                    return outcome.optBoolean("handled");
                }
            };
        }
    }
}
