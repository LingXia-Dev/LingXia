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
import com.lingxia.ui.generated.GpuInput;
import com.lingxia.ui.generated.GpuSemantics;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * GPU page content. Construct and control on the UI thread. The shared Rust host owns the View
 * worker, frame sequencing, input, image, bridge and recovery policy; this class converts Android
 * events into host calls and performs the typed effects the host sends through {@link GpuPlatform}.
 */
public final class GpuView extends FrameLayout implements SurfaceHolder.Callback2 {
    /** Standalone hosts call this after loading the library; the SDK initializes it automatically. */
    public static native void initializeNative();

    static native long nativeHostCreate(GpuPlatform platform, String worker, boolean logic, long imageOwner);
    static native void nativeHostRelease(long host);
    static native void nativeRunTask(long task);
    static native void nativeLoad(long host, String source);
    static native void nativeReload(long host);
    static native void nativePark(long host);
    static native void nativeSetVisible(long host, boolean visible);
    static native void nativeSurfaceChanged(long host);
    static native void nativeSurfaceLost(long host);
    static native void nativeRequestFrame(long host);
    static native void nativeRedraw(long host);
    static native boolean nativeFrameCurrent(long host);
    static native void nativeVsync(long host);
    static native void nativeRetry(long host);
    static native void nativeConnect(long host, long connection, String nonce);
    static native void nativeReceive(long host, long connection, String frame);
    static native void nativeRequestLeave(long host, long connection, String reason);
    static native long nativeConnection(long host);
    static native boolean nativeInputBlocked(long host);
    static native void nativeWindowFocus(long host, boolean focused);
    static native boolean nativeBridgeReady(long host);
    /** A platform-detected failure; the host classifies it and shows recovery. */
    static native void nativeFail(long host, String error);
    static native void nativeUpdateEnvironment(long host, String update);
    static native boolean nativePointer(long host, int phase, float x, float y, float slop, float vx, float vy, float minFling);
    static native void nativeLongPress(long host);
    static native boolean nativeKey(long host, int key, boolean shift, boolean repeat);
    static native void nativeCancel(long host);
    static native void nativeFocusEntered(long host, boolean backward);
    static native void nativeFocusLeft(long host);
    static native boolean nativeExplore(long host, boolean active, float x, float y);
    static native boolean nativeAccessibility(long host, int id, int action, double number, String argument);
    static native boolean nativeScrollBy(long host, String region, float delta);
    static native boolean nativeCanScrollVertically(long host, int direction);
    static native String nativeSemanticRoots(long host);
    static native String nativeSemanticNode(long host, int id);
    static native String nativeAutomate(long host, String request);
    static native void nativeMoveTextSelection(long host, String identity, int which, float x, float y);
    static native void nativeSetTextSelection(long host, String identity, int start, int end);
    static native void nativeCopyTextSelection(long host, long callback);
    static native void nativeClearTextSelection(long host);
    static native void nativeInput(long host, String request, long callback);
    static native void nativeCaret(long host, boolean eligible, boolean connected, String text, int anchor, int focus, int start, int end);
    // Chrome geometry over a snapshot node's JSON; the policy lives in the shared core.
    static native boolean nativeChromeContains(String node, float x, float y, boolean ownRadius);
    static native float[] nativeHandleEndpoint(String node, int which);
    static native float[] nativeHandleHit(String node, float x, float y, boolean collapsed);
    static native float[] nativeHandleDrag(String node, float x, float y);
    /** Where the GPU crash sentinel keeps its record; the first call in a process wins. */
    static native void nativeGpuSentinel(String directory);
    static native void nativeGpuActive(boolean active);
    private static native void nativeAttach(long host, android.view.Surface surface, int width, int height);

    public interface PullToRefresh { boolean onTouch(MotionEvent event); }
    private PullToRefresh pullToRefresh;
    public void setPullToRefresh(PullToRefresh callback) { pullToRefresh = callback; }
    public void clearPullToRefresh(PullToRefresh callback) { if (pullToRefresh == callback) pullToRefresh = null; }
    @Override public boolean canScrollVertically(int direction) {
        return !destroyed && nativeCanScrollVertically(host, direction);
    }
    public interface Listener {
        void onLoaded();
        void onFrame(String metadata);
        void onError(Throwable error);
        default void onMessage(long generation, String frame) { }
        default boolean onRestart(long generation) { return false; }
    }

    private final android.os.Handler main = new android.os.Handler(android.os.Looper.getMainLooper());
    private final GpuRecovery recovery = new GpuRecovery(this);
    // Zero after disposal.
    private long host;
    private TextView bootstrapMessage;
    private int bootstrapSurfaceImportance;
    private final Listener listener;
    private final UiSurface surface;
    private final GpuIme ime;
    private final View feedback;
    // CPU frames are drawn from a raster thread, which reads these too.
    private volatile boolean active, destroyed, attaching, gpuAttached;
    private boolean hasSurfaceFrame;
    // The presented frame as JSON, for the input method and its chrome.
    private JSONObject snapshot;
    private String snapshotText;
    private float frameScale;
    // Bumped when the surface is replaced; a capture only completes against its own surface.
    private long surfaceEpoch;
    private java.util.function.Consumer<Boolean> nextPresentation;
    private final java.util.ArrayList<Runnable> surfaceRedraws = new java.util.ArrayList<>();
    private int unoccludedWidth, unoccludedHeight;
    private final android.graphics.Rect chromeInsets = new android.graphics.Rect();
    private GpuTextMenu textMenu;
    private GpuSelectionHandles selectionHandles;
    private GpuStaticSelection staticSelection;
    private android.content.res.Configuration hostConfiguration;
    private org.json.JSONArray textScale;
    private float textScalePreference = Float.NaN, textScaleDensity = Float.NaN;
    private final java.util.HashMap<Long, java.util.function.Consumer<JSONObject>> inputReplies = new java.util.HashMap<>();
    private long nextInputReply;
    private boolean vsyncRequested;
    private final Choreographer.FrameCallback vsync = time -> {
        vsyncRequested = false;
        if (!destroyed) nativeVsync(host);
    };

    /** {@code worker} names the View worker this page shares with its app session's pages. */
    public GpuView(Context context, String source, String worker, Listener listener) {
        this(context, source, worker, 0, listener, false);
    }
    /** {@code imageOwner} names this page to the host's image loader; 0 loads no images. */
    public GpuView(Context context, String source, String worker, long imageOwner, Listener listener, boolean logic) {
        super(context);
        nativeGpuSentinel(context.getNoBackupFilesDir().getAbsolutePath());
        hostConfiguration = new android.content.res.Configuration(context.getResources().getConfiguration());
        this.listener = listener;
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
                if (destroyed) return;
                long callback = ++nextInputReply;
                inputReplies.put(callback, reply);
                nativeInput(host, request.toString(), callback);
            }
            @Override public void render(boolean rebuild) {
                if (destroyed) return;
                if (rebuild) nativeRequestFrame(host); else nativeRedraw(host);
            }
            private int autofilled = -1;
            @Override public void editorChanged(String identity) {
                if (android.os.Build.VERSION.SDK_INT < 26) return;
                android.view.autofill.AutofillManager autofill = getContext().getSystemService(android.view.autofill.AutofillManager.class);
                if (autofill == null) return;
                if (autofilled >= 0) { autofill.notifyViewExited(surface, autofilled); autofilled = -1; }
                JSONObject node = null;
                JSONArray nodes = identity == null || snapshot == null ? null : snapshot.optJSONArray("nodes");
                for (int i = 0; nodes != null && i < nodes.length(); i++)
                    if (identity.equals(nodes.optJSONObject(i).optString("identity"))) node = nodes.optJSONObject(i);
                if (node == null) return;
                autofilled = surface.autofillId(node);
                Rect bounds = surface.bounds(node);
                int[] origin = new int[2]; surface.getLocationOnScreen(origin);
                bounds.offset(origin[0], origin[1]);
                autofill.notifyViewEntered(surface, autofilled, bounds);
            }
            @Override public void caret(boolean eligible, String text, int anchor, int focus, int start, int end) {
                if (!destroyed) nativeCaret(host, eligible, text != null, text == null ? "" : text, anchor, focus, start, end);
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
            public boolean busy() { return false; }
            public void hideMenu() { textMenu.dismiss(); }
            public void showMenu() { textMenu.show(); }
        });
        feedback.getOverlay().add(selectionHandles);
        staticSelection = new GpuStaticSelection(surface, new GpuStaticSelection.Host() {
            public JSONObject frame() { return snapshot; }
            public float scale() { return frameScale; }
            public void move(String identity, int which, float x, float y) { if (!destroyed) nativeMoveTextSelection(host, identity, which, x, y); }
            public void select(String identity, int start, int end) { if (!destroyed) nativeSetTextSelection(host, identity, start, end); }
            public void copy() { copyTextSelection(); }
            public void clear() { if (!destroyed) nativeClearTextSelection(host); }
        });
        feedback.getOverlay().add(staticSelection);
        host = nativeHostCreate(new Platform(), worker, logic, imageOwner);
        ime.setCaretActive(false);
        surface.getHolder().addCallback(this);
        addView(surface, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        addView(feedback, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        load(source);
    }

    public void reload() {
        if (destroyed) return;
        prepareInstance();
        nativeReload(host);
    }
    public void reload(String source) {
        if (destroyed) return;
        load(source);
    }
    private void load(String source) {
        prepareInstance();
        nativeLoad(host, source);
    }
    /** A new module instance: drop chrome and frame state tied to the previous one. */
    private void prepareInstance() {
        snapshot = null; snapshotText = null;
        inputReplies.clear();
        if (surface.getParent() != this) {
            removeAllViews();
            addView(surface, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
            addView(feedback, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
        }
    }
    /**
     * Visibility reaches the host after the next layout, where this page sits as shown: an
     * environment sampled before that would describe the previous page's chrome.
     */
    public void resume() {
        nativeGpuActive(true);
        active = true; frozen = false; updateSurfaceVisibility();
        if (layoutPending) return;
        layoutPending = true;
        getViewTreeObserver().addOnPreDrawListener(new android.view.ViewTreeObserver.OnPreDrawListener() {
            @Override public boolean onPreDraw() {
                getViewTreeObserver().removeOnPreDrawListener(this);
                layoutPending = false;
                if (destroyed || !active) return true;
                syncVisibility();
                // Shown in the same frame when the target already holds a frame for this layout.
                if (!revealed && nativeFrameCurrent(host)) reveal();
                return true;
            }
        });
        invalidate();
    }
    private boolean layoutPending;
    /** API 29+: a hidden page fades its layer and keeps its surface while the runtime retains it. */
    private static final boolean KEEP_SURFACES = android.os.Build.VERSION.SDK_INT >= 29;
    private boolean surfaceRetained = KEEP_SURFACES, revealed = true, holdSurface;
    private static final float OFF_SCREEN = -100_000f;
    public static boolean keepsSurfaces() { return KEEP_SURFACES; }
    /** The runtime's retention decision for this page while it is hidden. */
    public void setSurfaceRetained(boolean retained) {
        surfaceRetained = KEEP_SURFACES && retained;
        updateSurfaceVisibility();
        if (surfaceRetained && !gpuAttached) attachGpu();
    }
    private final java.util.ArrayList<Runnable> revealWaiters = new java.util.ArrayList<>();
    /** A returning page whose frame is stale is shown after this long, never left under a wrong highlight. */
    private static final long REVEAL_DEADLINE_MS = 300;
    /** A first visit waits for its first frame; this only catches a page that never produces one. */
    private static final long FIRST_FRAME_WATCHDOG_MS = 2000;
    private int revealEpoch;
    private boolean everPresented;
    /** Stay hidden, so the page beneath remains on screen, until this page presents or fails or the deadline passes; then run [then]. */
    public void revealOnFirstFrame(Runnable then) {
        revealed = false; revealWaiters.add(then); updateSurfaceVisibility();
        int epoch = ++revealEpoch;
        // Only a surface that still holds this page's frame can be shown stale; a recreated
        // or released one waits for its first frame like a first visit.
        boolean returning = everPresented && hasSurfaceFrame;
        long delay = returning ? REVEAL_DEADLINE_MS : FIRST_FRAME_WATCHDOG_MS;
        main.postDelayed(new Runnable() { @Override public void run() {
            if (epoch != revealEpoch || revealed) return;
            // A stopped window renders nothing; the wait resumes with it.
            if (isAttachedToWindow() && getWindowVisibility() != View.VISIBLE) { main.postDelayed(this, delay); return; }
            // Never show a frame drawn for another size: until one is current the surface stays off
            // screen and the activity canvas, painted with the host's page floor, shows through.
            holdSurface = !nativeFrameCurrent(host);
            String page = Integer.toHexString(System.identityHashCode(this));
            if (returning) Log.w("LingXia.UI", "reveal deadline: page " + page
                + " shown before a current frame (attached=" + gpuAttached + ", stale=" + holdSurface + ")");
            else Log.e("LingXia.UI", "first-frame watchdog: page " + page + " never presented (attached=" + gpuAttached + ")");
            reveal();
        }}, delay);
    }
    private void reveal() {
        if (revealed) return;
        revealed = true; updateSurfaceVisibility();
        java.util.ArrayList<Runnable> waiters = new java.util.ArrayList<>(revealWaiters);
        revealWaiters.clear();
        for (Runnable waiter : waiters) waiter.run();
    }
    /** The surface still holds a presented frame, so showing the page needs no new frame. */
    public boolean hasLiveFrame() {
        return !destroyed && hasSurfaceFrame && surface.getHolder().getSurface().isValid() && nativeFrameCurrent(host);
    }
    @Override protected void onVisibilityChanged(View changedView, int visibility) {
        super.onVisibilityChanged(changedView, visibility);
        updateSurfaceVisibility();
    }
    @Override protected void onWindowVisibilityChanged(int visibility) {
        super.onWindowVisibilityChanged(visibility);
        syncVisibility();
        // Surfaces recreated while the window was hidden attach now, a kept hidden page included.
        if (visibility == View.VISIBLE && !gpuAttached) main.post(this::attachGpu);
    }
    /** A page being left keeps its last frame on screen but schedules nothing more. */
    private boolean frozen;
    public void freeze() { frozen = true; syncVisibility(); }
    private void syncVisibility() {
        if (!destroyed && host != 0 && !(active && layoutPending))
            nativeSetVisible(host, active && !frozen && isShown() && getWindowVisibility() == View.VISIBLE);
    }
    private void updateSurfaceVisibility() {
        if (surface == null) return;
        syncVisibility();
        // A hidden page hides its compositor layer as well as its view tree: by moving it off
        // screen while its surface is retained (SurfaceView ignores alpha), otherwise by releasing it.
        boolean shown = isShown();
        surface.setVisibility(shown && (active || surfaceRetained) ? View.VISIBLE : View.INVISIBLE);
        if (KEEP_SURFACES) surface.setTranslationX(active && revealed && !holdSurface ? 0f : OFF_SCREEN);
        setImportantForAccessibility(active ? View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
            : View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        if (ime != null) updateCaretActivity();
    }
    private void updateCaretActivity() {
        ime.setCaretActive(active && isShown() && !destroyed && hasWindowFocus() && surface.getHolder().getSurface().isValid());
    }
    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (ime != null) {
            if (!focused) surface.cancelRefreshGesture();
            if (!destroyed) nativeWindowFocus(host, focused);
            updateCaretActivity();
        }
    }
    private void cancel() { if (!destroyed) nativeCancel(host); }
    private void requestFrame() { if (!destroyed) nativeRequestFrame(host); }
    public long getConnectionGeneration() { return destroyed ? 0 : nativeConnection(host); }
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
        final long captured = surfaceEpoch;
        try {
            android.view.PixelCopy.request(surface, capture, result -> {
                if (result == android.view.PixelCopy.SUCCESS && !destroyed && surfaceEpoch == captured) {
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
        nativeGpuActive(false);
        surface.cancelRefreshGesture(); finishSurfaceRedraws();
        active = false; frozen = false; updateSurfaceVisibility(); ime.clear(true);
        cancel();
    }
    public void park() {
        if (destroyed) return;
        pause(); snapshot = null; snapshotText = null;
        nativePark(host);
    }

    /** One automation request (query or act) on the presented frame: `{"value":...}` or `{"error":...}`. */
    public String automate(String request) {
        if (destroyed) return "{\"error\":\"page is not active: the GPU page was disposed\"}";
        return nativeAutomate(host, request);
    }

    private void writeClipboard(String text) {
        android.content.ClipboardManager clipboard = (android.content.ClipboardManager)getContext().getSystemService(Context.CLIPBOARD_SERVICE);
        try { if (clipboard != null) clipboard.setPrimaryClip(android.content.ClipData.newPlainText("", text)); }
        catch (SecurityException | IllegalStateException unavailable) { Log.w("LingXia.UI", "clipboard unavailable", unavailable); }
    }

    /** Copy the static-text selection to the clipboard; the core then drops it. */
    private void copyTextSelection() {
        if (destroyed) return;
        long callback = ++nextInputReply;
        inputReplies.put(callback, reply -> {
            String text = reply == null || reply.isNull("text") ? null : reply.optString("text");
            if (text != null && !text.isEmpty()) writeClipboard(text);
        });
        nativeCopyTextSelection(host, callback);
    }

    public void connect(long generation, String nonce) { if (!destroyed) nativeConnect(host, generation, nonce); }
    public void receive(long generation, String frame) { if (!destroyed) nativeReceive(host, generation, frame); }
    public void requestLeave(long generation, String reason) { if (!destroyed) nativeRequestLeave(host, generation, reason); }

    private void showBootstrapMessage(boolean overdue) {
        if (!overdue) {
            if (bootstrapMessage != null) {
                removeView(bootstrapMessage);
                bootstrapMessage = null;
                surface.setImportantForAccessibility(bootstrapSurfaceImportance);
            }
            return;
        }
        if (bootstrapMessage != null) return;
        bootstrapMessage = new TextView(getContext());
        bootstrapMessage.setText("This page is taking longer to load. You can go back and try again.");
        bootstrapMessage.setTextColor(android.graphics.Color.BLACK);
        bootstrapMessage.setBackgroundColor(android.graphics.Color.WHITE);
        bootstrapMessage.setPadding(32, 48, 32, 32);
        bootstrapMessage.setClickable(true);
        bootstrapSurfaceImportance = surface.getImportantForAccessibility();
        surface.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        addView(bootstrapMessage, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));
    }
    private final android.database.ContentObserver animatorScale = new android.database.ContentObserver(main) {
        @Override public void onChange(boolean selfChange) { requestFrame(); }
    };
    @Override protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        getContext().getContentResolver().registerContentObserver(
            android.provider.Settings.Global.getUriFor(android.provider.Settings.Global.ANIMATOR_DURATION_SCALE), false, animatorScale);
        requestFrame();
    }
    @Override protected void onDetachedFromWindow() {
        getContext().getContentResolver().unregisterContentObserver(animatorScale);
        super.onDetachedFromWindow();
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
        nativeUpdateEnvironment(host, update);
    }

    /** Measured overlap in physical pixels, supplied by the native host after layout. */
    public void setChromeInsets(int left, int top, int right, int bottom) {
        if (destroyed || (chromeInsets.left == left && chromeInsets.top == top
                && chromeInsets.right == right && chromeInsets.bottom == bottom)) return;
        chromeInsets.set(left, top, right, bottom);
        recovery.setInsets(chromeInsets);
        requestFrame();
    }

    /** Raw device facts for the next frame (device px); the core builds the environment. Null while the surface cannot present. */
    private String sampleEnvironment() {
        android.view.Surface target = surface.getHolder().getSurface();
        if (destroyed || target == null || !target.isValid()) return null;
        if (surface.getWidth() <= 0 || surface.getHeight() <= 0) return null;
        return measureFacts();
    }

    private String measureFacts() {
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
            // Window insets are relative to the root; report their overlap with this surface.
            int[] origin = new int[2], rootOrigin = new int[2];
            // The surface fills this view; a hidden page moves only the surface off screen,
            // so measure from the view or the layout would take that offset as an inset.
            getLocationOnScreen(origin); getRootView().getLocationOnScreen(rootOrigin);
            int x = origin[0] - rootOrigin[0], y = origin[1] - rootOrigin[1];
            int trailingX = getRootView().getWidth() - x - surface.getWidth();
            int trailingY = getRootView().getHeight() - y - surface.getHeight();
            android.content.res.Configuration configuration = hostConfiguration;
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
            return new JSONObject()
                .put("width", surface.getWidth()).put("height", surface.getHeight()).put("scale", (double)density)
                .put("dark", (configuration.uiMode & android.content.res.Configuration.UI_MODE_NIGHT_MASK)
                    == android.content.res.Configuration.UI_MODE_NIGHT_YES)
                .put("system_insets", new JSONArray().put(top - y).put(right - trailingX).put(bottom - trailingY).put(left - x))
                .put("chrome_insets", new JSONArray().put(chromeInsets.top).put(chromeInsets.right).put(chromeInsets.bottom).put(chromeInsets.left))
                .put("keyboard", keyboard - trailingY)
                .put("smallest_width", configuration.smallestScreenWidthDp)
                .put("locale", locale.toLanguageTag())
                .put("fine_pointer", false)
                .put("animation_scale", (double)android.provider.Settings.Global.getFloat(
                    getContext().getContentResolver(), android.provider.Settings.Global.ANIMATOR_DURATION_SCALE, 1f))
                .put("reduce_motion", false)
                .put("text_scale", textScale)
                .put("runner", false).toString();
        } catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native environment", error); }
    }
    public void dispose() {
        if (destroyed) return;
        nextPresentation = null;
        finishSurfaceRedraws();
        showBootstrapMessage(false);
        recovery.clear();
        cancel();
        destroyed = true;
        inputReplies.clear();
        ime.clear(false); surface.stopFling();
        if (vsyncRequested) Choreographer.getInstance().removeFrameCallback(vsync);
        long released = host;
        host = 0;
        nativeHostRelease(released);
    }
    @Override public void surfaceCreated(SurfaceHolder holder) { updateCaretActivity(); }
    @Override public void surfaceDestroyed(SurfaceHolder holder) {
        surface.cancelRefreshGesture();
        hasSurfaceFrame = false;
        surfaceEpoch++;
        finishSurfaceRedraws();
        ime.setCaretActive(false);
        gpuAttached = false;
        if (!destroyed) nativeSurfaceLost(host);
    }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        hasSurfaceFrame = false;
        surfaceEpoch++;
        if (destroyed) return;
        nativeSurfaceChanged(host);
        if (!gpuAttached) attachGpu();
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
        // A retained hidden page attaches too, so its catch-up frame is ready when it returns.
        android.view.Surface target = surface.getHolder().getSurface();
        if (attaching || destroyed || !(active || surfaceRetained) || !isShown()
            || target == null || !target.isValid()) return;
        int width = surface.getWidth(), height = surface.getHeight();
        if (width <= 0 || height <= 0) return;
        attaching = true;
        try { nativeAttach(host, surface.getHolder().getSurface(), width, height); }
        catch (UnsatisfiedLinkError cpuOnly) { attaching = false; }
    }

    private void showFailure(int message, int action, boolean retained, String error) {
        if (message < 0) { recovery.clear(); return; }
        reveal();
        finishPresentation(false);
        Log.e("LingXia.UI", "Render failed: " + error);
        if (listener != null) listener.onError(new IllegalStateException(error));
        recovery.show(retained, message, action, action < 0 ? null : () -> { if (!destroyed) nativeRetry(host); });
    }

    /** Runs only on the main thread, as the host's platform. */
    private final class Platform implements GpuPlatform {
        @Override public void post(long task, long delayMillis) {
            main.postDelayed(() -> nativeRunTask(task), delayMillis);
        }
        @Override public void requestVsync() {
            if (destroyed || vsyncRequested) return;
            vsyncRequested = true;
            Choreographer.getInstance().postFrameCallback(vsync);
        }
        @Override public String environment() { return sampleEnvironment(); }
        @Override public boolean presentPixels(int[] argb, int width, int height) {
            if (!active || !surface.getHolder().getSurface().isValid()) return false;
            Canvas canvas = surface.getHolder().lockCanvas();
            if (canvas == null) return false;
            Bitmap frame = Bitmap.createBitmap(argb, width, height, Bitmap.Config.ARGB_8888);
            try { canvas.drawBitmap(frame, 0, 0, null); }
            finally { surface.getHolder().unlockCanvasAndPost(canvas); frame.recycle(); }
            return true;
        }
        @Override public void gpuAttached(boolean attached) {
            attaching = false;
            if (destroyed) return;
            gpuAttached = attached;
            if (!destroyed) nativeRedraw(host);
        }
        @Override public void announce(int event, int id) { surface.announce(event, id); }
        @Override public void announceText(int id, boolean secure, String before, String after) {
            if (!surface.accessibilityEnabled()) return;
            AccessibilityEvent event = surface.event(id, AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED);
            event.setClassName("android.widget.EditText"); event.setPassword(secure);
            event.setBeforeText(before); event.getText().add(after); event.setFromIndex(0);
            event.setRemovedCount(before.length()); event.setAddedCount(after.length());
            surface.send(event);
        }
        @Override public void announceSelection(int id, boolean secure, String text, int start, int end) {
            if (!surface.accessibilityEnabled()) return;
            AccessibilityEvent event = surface.event(id, AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
            event.setClassName("android.widget.EditText"); event.setPassword(secure);
            event.getText().add(text); event.setFromIndex(start); event.setToIndex(end);
            event.setItemCount(text.length());
            surface.send(event);
        }
        @Override public void announceClicked() { surface.sendAccessibilityEvent(AccessibilityEvent.TYPE_VIEW_CLICKED); }
        @Override public void announceLive(int id, String text, boolean assertive) {
            if (!surface.accessibilityEnabled()) return;
            if (assertive) { surface.announceForAccessibility(text); return; }
            AccessibilityEvent event = surface.event(id, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
            event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_TEXT);
            event.getText().add(text);
            surface.send(event);
        }
        @Override public void frame(String encoded, float scale) {
            try { snapshot = new JSONObject(encoded); }
            catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native frame", error); }
            snapshotText = encoded; frameScale = scale; hasSurfaceFrame = true;
            ime.presented(snapshot, ime.capture());
            staticSelection.setBounds(0, 0, feedback.getWidth(), feedback.getHeight());
            staticSelection.update();
            updateCaretActivity();
        }
        @Override public void imeFocus(int index, long commit, boolean touch, float x, float y, boolean word) {
            JSONObject node = surface.node(index);
            if (node == null) return;
            if (touch) ime.touch(node, commit, x, y, word); else ime.focus(node, commit);
        }
        @Override public void imeActivation() { if (ime.isActive()) ime.clear(true); }
        @Override public void imeReset() { ime.reset(); }
        @Override public void inputReply(long callback, String reply) {
            java.util.function.Consumer<JSONObject> consumer = inputReplies.remove(callback);
            if (consumer == null || destroyed) return;
            JSONObject result = null;
            try {
                Object value = reply == null ? null : new org.json.JSONTokener(reply).nextValue();
                if (value instanceof JSONObject) result = (JSONObject)value;
            } catch (org.json.JSONException error) { Log.e("LingXia.UI", "Invalid input reply", error); }
            consumer.accept(result);
        }
        @Override public void press(int node) { surface.showPress(node); }
        @Override public void clearPress() { surface.clearPress(); }
        @Override public void focusRing(int node) { surface.showFocusRing(node); }
        @Override public void clearFocusRing() { surface.clearFocusRing(); }
        @Override public void claim(boolean exclusive) {
            if (surface.getParent() != null) surface.getParent().requestDisallowInterceptTouchEvent(exclusive);
        }
        @Override public void armLongPress() { surface.armLongPress(); }
        @Override public void cancelLongPress() { surface.disarmLongPress(); }
        @Override public void fling(String region, boolean horizontal, float velocity) { surface.fling(region, horizontal, velocity); }
        @Override public void stopFling() { surface.stopFling(); }
        @Override public void clicked() { surface.performClick(); }
        @Override public void haptic() { surface.performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS); }
        @Override public void editText(int index, String text, int start, int end) {
            JSONObject node = surface.node(index);
            if (node != null && snapshot != null) ime.accessibilityEdit(node, snapshot.optLong("generation"), text, start, end);
        }
        @Override public void loaded() { if (listener != null) listener.onLoaded(); }
        @Override public void presented() {
            if (listener != null && snapshotText != null) listener.onFrame(snapshotText);
            everPresented = true;
            if (holdSurface) { holdSurface = false; updateSurfaceVisibility(); }
            reveal();
            finishPresentation(true);
        }
        @Override public void message(long connection, String message) {
            if (listener != null) listener.onMessage(connection, message);
        }
        @Override public boolean restart(long connection) { return listener != null && listener.onRestart(connection); }
        @Override public void recovery(int message, int action, boolean retained, String error) {
            showFailure(message, action, retained, error);
        }
        @Override public void bootstrapOverdue(boolean overdue) { showBootstrapMessage(overdue); }
        @Override public void copyText(String text) { if (!destroyed) writeClipboard(text); }
    }

    private static final int[] KEYS = {KeyEvent.KEYCODE_TAB, KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN,
        KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_MOVE_HOME, KeyEvent.KEYCODE_MOVE_END};
    private static int key(int code) {
        for (int i = 0; i < KEYS.length; i++) if (KEYS[i] == code) return i;
        return code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_SPACE || code == KeyEvent.KEYCODE_DPAD_CENTER ? 7 : -1;
    }

    private static JSONObject json(String encoded) {
        try { return "null".equals(encoded) ? null : new JSONObject(encoded); }
        catch (org.json.JSONException error) { throw new IllegalStateException("Invalid native reply", error); }
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
        private final Runnable longPress = () -> { if (!destroyed) nativeLongPress(host); };
        private final Choreographer.FrameCallback flingFrame = new Choreographer.FrameCallback() {
            @Override public void doFrame(long time) {
                if (!active || destroyed || flingRegion == null || !scroller.computeScrollOffset()) return;
                int position = flingHorizontal ? scroller.getCurrX() : scroller.getCurrY();
                boolean changed = nativeScrollBy(host, flingRegion, position - flingPosition);
                flingPosition = position;
                if (!changed) { scroller.abortAnimation(); flingRegion = null; return; }
                Choreographer.getInstance().postFrameCallback(this);
            }
        };

        UiSurface() {
            super(GpuView.this.getContext());
            slop = ViewConfiguration.get(getContext()).getScaledTouchSlop();
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
            setFocusable(true);
            setFocusableInTouchMode(true);
            if (android.os.Build.VERSION.SDK_INT >= 26) {
                setDefaultFocusHighlightEnabled(false);
                setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
            }
            pressedOverlay.setColor(0x22000000);
            focusOverlay.setColor(Color.TRANSPARENT);
        }
        JSONObject node(int index) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            return nodes == null ? null : nodes.optJSONObject(index);
        }
        /** Autofill addresses an editor by a virtual id stable across frames. */
        int autofillId(JSONObject node) { return node.optString("identity").hashCode() & 0x7fffffff; }
        JSONObject editor(int autofillId) {
            JSONArray nodes = snapshot == null ? null : snapshot.optJSONArray("nodes");
            for (int i = 0; nodes != null && i < nodes.length(); i++) {
                JSONObject node = nodes.optJSONObject(i);
                if (node != null && node.optJSONObject("input") != null && autofillId(node) == autofillId) return node;
            }
            return null;
        }
        @Override public void onProvideAutofillVirtualStructure(android.view.ViewStructure structure, int flags) {
            if (android.os.Build.VERSION.SDK_INT < 26) return;
            structure.setClassName(getClass().getName());
            JSONArray nodes = snapshot == null || !active ? null : snapshot.optJSONArray("nodes");
            java.util.List<JSONObject> editors = new java.util.ArrayList<>();
            for (int i = 0; nodes != null && i < nodes.length(); i++) {
                JSONObject node = nodes.optJSONObject(i);
                if (GpuIme.editable(node)) editors.add(node);
            }
            int index = structure.addChildCount(editors.size());
            for (JSONObject node : editors) {
                JSONObject input = node.optJSONObject("input");
                android.view.ViewStructure child = structure.newChild(index++);
                child.setAutofillId(structure.getAutofillId(), autofillId(node));
                child.setClassName("android.widget.EditText");
                child.setAutofillType(View.AUTOFILL_TYPE_TEXT);
                JSONObject edit = input.optJSONObject("edit");
                String value = edit != null ? edit.optString("text") : input.optString("value");
                child.setAutofillValue(android.view.autofill.AutofillValue.forText(value));
                JSONObject ime = input.optJSONObject("ime") == null ? new JSONObject() : input.optJSONObject("ime");
                String hint = ime.isNull("auto_complete") ? null : GpuInput.autofillHint(ime.optString("auto_complete"));
                if (hint != null) child.setAutofillHints(new String[]{hint});
                child.setInputType(GpuIme.inputType(ime));
                child.setHint(input.optString("placeholder"));
                child.setDataIsSensitive(!value.isEmpty());
                Rect rect = bounds(node);
                child.setDimens(rect.left, rect.top, 0, 0, rect.width(), rect.height());
                child.setVisibility(View.VISIBLE);
                child.setEnabled(input.optBoolean("enabled", true));
            }
        }
        @Override public void autofill(android.util.SparseArray<android.view.autofill.AutofillValue> values) {
            if (android.os.Build.VERSION.SDK_INT < 26 || destroyed || snapshot == null) return;
            for (int i = 0; i < values.size(); i++) {
                android.view.autofill.AutofillValue value = values.valueAt(i);
                JSONObject node = editor(values.keyAt(i));
                if (node != null && value != null && value.isText())
                    ime.accessibilityEdit(node, snapshot.optLong("generation"), value.getTextValue().toString(), -1, -1);
            }
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
        @Override public boolean onTouchEvent(MotionEvent event) {
            if (destroyed || !active || nativeInputBlocked(host)) return false;
            if (staticSelection.touch(event)) {
                cancel();
                getParent().requestDisallowInterceptTouchEvent(staticSelection.dragging());
                return true;
            }
            if (selectionHandles.touch(event)) {
                cancel();
                getParent().requestDisallowInterceptTouchEvent(selectionHandles.dragging());
                return true;
            }
            int action = event.getActionMasked();
            if (action == MotionEvent.ACTION_DOWN) refreshingGesture = false;
            boolean refreshConsumed = pullToRefresh != null && pullToRefresh.onTouch(event);
            if (refreshConsumed || refreshingGesture) {
                if (!refreshingGesture) {
                    cancel();
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
            boolean handled = nativePointer(host, phase, event.getX(), event.getY(), slop, vx, vy,
                configuration.getScaledMinimumFlingVelocity());
            if (phase >= 2 && velocity != null) { velocity.recycle(); velocity = null; }
            if (phase == 0 && !handled) return pullToRefresh != null;
            return true;
        }
        @Override public boolean performClick() { super.performClick(); return true; }
        @Override public boolean onCheckIsTextEditor() { return ime != null && ime.hasFocus(); }
        @Override public android.view.inputmethod.InputConnection onCreateInputConnection(android.view.inputmethod.EditorInfo info) {
            return ime == null ? null : ime.create(info);
        }
        @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
            if (destroyed || nativeInputBlocked(host)) return super.onKeyDown(keyCode, event);
            if (ime.hasFocus() && keyCode != KeyEvent.KEYCODE_TAB) return ime.key(event) || super.onKeyDown(keyCode, event);
            if (ime.hasFocus()) ime.clear(true);
            int key = key(keyCode);
            if (key >= 0 && nativeKey(host, key, event.isShiftPressed(), event.getRepeatCount() > 0)) return true;
            return super.onKeyDown(keyCode, event);
        }
        @Override public boolean onKeyShortcut(int keyCode, KeyEvent event) {
            if (destroyed || nativeInputBlocked(host)) return super.onKeyShortcut(keyCode, event);
            return ime.hasFocus() ? ime.key(event) : super.onKeyShortcut(keyCode, event);
        }
        @Override protected void onFocusChanged(boolean focused, int direction, Rect previouslyFocusedRect) {
            super.onFocusChanged(focused, direction, previouslyFocusedRect);
            if (destroyed) return;
            if (!focused) { nativeFocusLeft(host); if (ime != null) ime.clear(true); }
            else if (!isInTouchMode() && (ime == null || !ime.hasFocus()))
                nativeFocusEntered(host, direction == View.FOCUS_BACKWARD || direction == View.FOCUS_UP);
        }
        @Override public boolean dispatchHoverEvent(MotionEvent event) {
            android.view.accessibility.AccessibilityManager manager = (android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            if (!active || destroyed || manager == null || !manager.isEnabled() || !manager.isTouchExplorationEnabled()) {
                if (!destroyed) nativeExplore(host, false, 0, 0);
                return super.dispatchHoverEvent(event);
            }
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_HOVER_ENTER:
                case MotionEvent.ACTION_HOVER_MOVE: return nativeExplore(host, true, event.getX(), event.getY());
                case MotionEvent.ACTION_HOVER_EXIT: nativeExplore(host, false, 0, 0); return true;
                default: return false;
            }
        }
        boolean accessibilityEnabled() {
            android.view.accessibility.AccessibilityManager manager = (android.view.accessibility.AccessibilityManager)getContext().getSystemService(Context.ACCESSIBILITY_SERVICE);
            return getParent() != null && manager != null && manager.isEnabled();
        }
        AccessibilityEvent event(int id, int type) {
            AccessibilityEvent event = AccessibilityEvent.obtain(type);
            // Virtual descendants must be invalidated, not only the host node.
            if (type == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED) event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
            event.setSource(this, id); event.setPackageName(getContext().getPackageName());
            return event;
        }
        void send(AccessibilityEvent event) { getParent().requestSendAccessibilityEvent(this, event); }
        private final int[] EVENTS = {AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED,
            AccessibilityEvent.TYPE_VIEW_FOCUSED, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT,
            AccessibilityEvent.TYPE_VIEW_SELECTED, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED};
        /** Translate a core accessibility event into an Android event. */
        void announce(int kind, int id) {
            if (!accessibilityEnabled() || kind < 0 || kind >= EVENTS.length) return;
            send(event(kind == 6 ? View.NO_ID : id, EVENTS[kind]));
        }
        @Override public AccessibilityNodeProvider getAccessibilityNodeProvider() {
            return new AccessibilityNodeProvider() {
                private JSONObject roots() { return destroyed ? null : json(nativeSemanticRoots(host)); }
                private JSONObject semantic(int id) { return destroyed ? null : json(nativeSemanticNode(host, id)); }
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
                    return nativeAccessibility(host, id, shared, number, argument);
                }
            };
        }
    }
}
