package com.lingxia.webview;

import android.app.Activity;
import android.content.Context;
import android.content.ContextWrapper;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.SurfaceTexture;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.DisplayMetrics;
import android.view.Choreographer;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.PixelCopy;
import android.view.Surface;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.webkit.ValueCallback;
import android.widget.FrameLayout;
import java.io.ByteArrayOutputStream;
import java.lang.ref.WeakReference;
import java.util.ArrayDeque;
import java.util.concurrent.ConcurrentHashMap;
import org.json.JSONObject;

/** Android view host for Servo's Rust embedding API. */
public final class LingXiaServoView extends FrameLayout implements LingXiaWebViewHost,
        TextureView.SurfaceTextureListener, Choreographer.FrameCallback {
    public interface NativeComponentMessageHandler {
        void onMessage(String message);
        void onDestroyed();
    }

    public interface EmbedderControlHandler {
        void show(long requestId, String kind, String payload);
        void hide(long requestId);
        void onDestroyed();
    }

    /** Sees each touch before Servo; returning true takes the rest of the gesture. */
    public interface TouchInterceptor {
        boolean onTouch(MotionEvent event);
    }

    private static final int MAX_PENDING_COMPONENT_MESSAGES = 128;
    private static final String TAG = "LingXiaServoView";
    private static final ConcurrentHashMap<String, WeakReference<LingXiaServoView>> sViews =
            new ConcurrentHashMap<>();
    private static final java.util.concurrent.atomic.AtomicLong sWindowReleaseSeq =
            new java.util.concurrent.atomic.AtomicLong();
    /** Surfaces Servo may still render into, freed once it unbinds them. */
    private static final ConcurrentHashMap<Long, PendingWindowRelease> sPendingWindowReleases =
            new ConcurrentHashMap<>();

    private static final class PendingWindowRelease {
        final Surface surface;
        final SurfaceTexture texture;

        PendingWindowRelease(Surface surface, SurfaceTexture texture) {
            this.surface = surface;
            this.texture = texture;
        }

        void release() {
            if (surface != null) surface.release();
            if (texture != null) texture.release();
        }
    }
    private final TextureView servoSurface;
    private final Editable editable = new SpannableStringBuilder();
    private final ServoInputConnection inputConnection;
    private Surface nativeSurface;
    private String servoWebTag;
    private String appId;
    private String currentPath;
    private long sessionId;
    private long nativeViewId;
    private boolean strictSecurityProfile = true;
    /** Servo renders into a window made from {@link #retainedTexture}. */
    private boolean attached;
    /**
     * The texture Servo renders into. It is kept across detach from the window:
     * rebinding it on reattach keeps the EGL surface, and the document with it.
     */
    private SurfaceTexture retainedTexture;
    /** The retained texture is on screen, so frames may be produced. */
    private boolean textureShown;
    private boolean destroyed;
    private boolean frameScheduled;
    private boolean paused;
    private boolean composing;
    private boolean touchIntercepted;
    private int contentScrollX;
    private int contentScrollY;
    private String composingText = "";
    private int editorInputType = InputType.TYPE_CLASS_TEXT;
    private int editorImeOptions = EditorInfo.IME_ACTION_DONE;
    private final ArrayDeque<String> pendingComponentMessages = new ArrayDeque<>();
    private NativeComponentMessageHandler nativeComponentMessageHandler;
    private EmbedderControlHandler embedderControlHandler;
    private TouchInterceptor touchInterceptor;

    public LingXiaServoView(Context context) {
        super(context);
        setBackgroundColor(Color.TRANSPARENT);
        setFocusable(true);
        setFocusableInTouchMode(true);
        inputConnection = new ServoInputConnection();
        servoSurface = new TextureView(context);
        servoSurface.setSurfaceTextureListener(this);
        servoSurface.setFocusable(true);
        servoSurface.setFocusableInTouchMode(true);
        addView(servoSurface, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
    }

    void initialize(
            String appId,
            String path,
            long sessionId,
            long nativeViewId,
            boolean strictSecurityProfile) {
        if (nativeViewId <= 0) {
            throw new IllegalArgumentException("nativeViewId must be positive");
        }
        this.appId = appId;
        this.currentPath = path;
        this.sessionId = sessionId;
        this.nativeViewId = nativeViewId;
        this.strictSecurityProfile = strictSecurityProfile;
        servoWebTag = appId + ":" + path + (sessionId > 0 ? "#" + sessionId : "");
        sViews.put(servoWebTag, new WeakReference<>(this));
        SurfaceTexture texture = servoSurface.getSurfaceTexture();
        if (servoSurface.isAvailable() && texture != null
                && servoSurface.getWidth() > 0 && servoSurface.getHeight() > 0) {
            attachNativeSurface(texture, servoSurface.getWidth(), servoSurface.getHeight());
        }
    }

    /** Rust binds every ready WebView; a Servo view already carries its identity. */
    public void setNativeViewId(long nativeViewId) {
        if (nativeViewId != this.nativeViewId) {
            throw new IllegalStateException("nativeViewId is immutable for this WebView");
        }
    }

    @Override
    public long getNativeViewId() {
        return nativeViewId;
    }

    private boolean bound() {
        return servoWebTag != null && nativeViewId > 0;
    }

    private void attachNativeSurface(SurfaceTexture texture, int width, int height) {
        if (retainedTexture != null && retainedTexture != texture) {
            // The view lost the retained texture; Servo rebinds to the new one.
            releaseNativeSurface(true);
        }
        retainedTexture = texture;
        if (nativeSurface != null && !attached) {
            nativeSurface.release();
            nativeSurface = null;
        }
        if (nativeSurface == null) nativeSurface = new Surface(texture);
        textureShown = true;
        createNativeSurface(nativeSurface, width, height);
    }

    /**
     * Hand the window to Servo for unbinding. Its buffers stay alive until the
     * engine confirms it stopped rendering into them, never by a UI timeout.
     */
    private void releaseNativeSurface(boolean releaseTexture) {
        PendingWindowRelease release = new PendingWindowRelease(
                nativeSurface, releaseTexture ? retainedTexture : null);
        nativeSurface = null;
        if (releaseTexture) retainedTexture = null;
        boolean deferred = false;
        if (attached && bound()) {
            long token = sWindowReleaseSeq.incrementAndGet();
            sPendingWindowReleases.put(token, release);
            deferred = nativeSurfaceDestroyed(servoWebTag, nativeViewId, token);
            if (!deferred) sPendingWindowReleases.remove(token);
        }
        attached = false;
        frameScheduled = false;
        if (!deferred) release.release();
    }

    /** Called by the engine once it no longer renders into the window. */
    static void onWindowReleased(final long token) {
        runOnMainThread(() -> {
            PendingWindowRelease release = sPendingWindowReleases.remove(token);
            if (release != null) release.release();
        });
    }

    @Override
    protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        if (retainedTexture != null && servoSurface.getSurfaceTexture() != retainedTexture) {
            servoSurface.setSurfaceTexture(retainedTexture);
        }
        if (retainedTexture != null && !textureShown) {
            textureShown = true;
            if (bound() && attached) nativeSetSurfaceShown(servoWebTag, nativeViewId, true);
            scheduleFrame();
        }
    }

    private float density() {
        DisplayMetrics metrics = getResources().getDisplayMetrics();
        return metrics != null ? metrics.density : 1.0f;
    }

    private void createNativeSurface(Surface surface, int width, int height) {
        if (!bound() || attached) return;
        nativeSurfaceCreated(servoWebTag, nativeViewId, surface, width, height, density());
        attached = true;
        scheduleFrame();
    }

    private void scheduleFrame() {
        if (!frameScheduled && attached && textureShown && !paused) {
            frameScheduled = true;
            Choreographer.getInstance().postFrameCallback(this);
        }
    }

    @Override
    public void onSurfaceTextureAvailable(SurfaceTexture texture, int width, int height) {
        attachNativeSurface(texture, width, height);
    }

    @Override
    public void onSurfaceTextureSizeChanged(SurfaceTexture texture, int width, int height) {
        if (bound() && attached) nativeSurfaceChanged(servoWebTag, nativeViewId, width, height);
    }

    @Override
    public boolean onSurfaceTextureDestroyed(SurfaceTexture texture) {
        if (texture != retainedTexture) {
            // Already handed to the engine for release, or never ours.
            return !isPendingRelease(texture);
        }
        if (!destroyed) {
            // Detached from the window only: keep the texture for reattach and
            // stop producing frames nobody can consume.
            textureShown = false;
            if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
            frameScheduled = false;
            if (bound() && attached) nativeSetSurfaceShown(servoWebTag, nativeViewId, false);
            return false;
        }
        releaseNativeSurface(true);
        return false;
    }

    private static boolean isPendingRelease(SurfaceTexture texture) {
        for (PendingWindowRelease release : sPendingWindowReleases.values()) {
            if (release.texture == texture) return true;
        }
        return false;
    }

    @Override
    public void onSurfaceTextureUpdated(SurfaceTexture texture) {}

    @Override
    public void doFrame(long frameTimeNanos) {
        frameScheduled = false;
        if (bound() && attached && textureShown) nativeFrame(servoWebTag, nativeViewId);
        scheduleFrame();
    }

    @Override
    public View getHostView() {
        return this;
    }

    @Override
    public String getAppId() {
        return appId;
    }

    @Override
    public String getCurrentPath() {
        return currentPath;
    }

    @Override
    public long getSessionId() {
        return sessionId;
    }

    @Override
    public String getUrl() {
        return bound() ? nativeGetUrl(servoWebTag, nativeViewId) : "";
    }

    @Override
    public String getTitle() {
        return bound() ? nativeGetTitle(servoWebTag, nativeViewId) : "";
    }

    @Override
    public boolean canGoBack() {
        return bound() && nativeCanGoBack(servoWebTag, nativeViewId);
    }

    @Override
    public boolean canGoForward() {
        return bound() && nativeCanGoForward(servoWebTag, nativeViewId);
    }

    @Override
    public boolean usesStrictSecurityProfile() {
        return strictSecurityProfile;
    }

    @Override
    public boolean retainsSurfaceWhenHidden() {
        return true;
    }

    /** Document scroll lives inside Servo; overlays follow the reported offset. */
    @Override
    public int getContentScrollX() {
        return contentScrollX;
    }

    @Override
    public int getContentScrollY() {
        return contentScrollY;
    }

    @Override
    public boolean canScrollVertically(int direction) {
        return direction < 0 ? contentScrollY > 0 : super.canScrollVertically(direction);
    }

    @Override
    public void reload() {
        if (bound()) nativeNavigate(servoWebTag, nativeViewId, 0);
    }

    @Override
    public void goBack() {
        if (bound()) nativeNavigate(servoWebTag, nativeViewId, 1);
    }

    @Override
    public void goForward() {
        if (bound()) nativeNavigate(servoWebTag, nativeViewId, 2);
    }

    @Override
    public void evaluateJavascript(String script, ValueCallback<String> callback) {
        if (!bound()) {
            if (callback != null) callback.onReceiveValue("null");
            return;
        }
        long requestId = LingXiaWebView.registerServoEvaluation(servoWebTag, callback);
        nativeEvaluate(servoWebTag, nativeViewId, requestId, script);
    }

    public void setTouchInterceptor(TouchInterceptor interceptor) {
        touchInterceptor = interceptor;
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        if (!bound()) return false;
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) {
            touchIntercepted = false;
            requestFocus();
        }
        TouchInterceptor interceptor = touchInterceptor;
        if (interceptor != null && interceptor.onTouch(event)) {
            if (!touchIntercepted) {
                // Servo already saw this gesture begin; end it there.
                touchIntercepted = true;
                int index = event.getActionIndex();
                nativeTouch(servoWebTag, nativeViewId, MotionEvent.ACTION_CANCEL,
                        event.getPointerId(index), event.getX(index), event.getY(index));
            }
            return true;
        }
        if (touchIntercepted) {
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                touchIntercepted = false;
            }
            return true;
        }
        int index = event.getActionIndex();
        nativeTouch(
                servoWebTag,
                nativeViewId,
                action,
                event.getPointerId(index),
                event.getX(index),
                event.getY(index));
        return true;
    }

    @Override
    public boolean dispatchTouchEvent(MotionEvent event) {
        return onTouchEvent(event);
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        outAttrs.inputType = editorInputType;
        outAttrs.imeOptions = editorImeOptions;
        outAttrs.initialSelStart = Selection.getSelectionStart(editable);
        outAttrs.initialSelEnd = Selection.getSelectionEnd(editable);
        return inputConnection;
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (bound() && forwardKeyEvent(event)) return true;
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void dispatchClickAt(float x, float y) {
        if (!bound()) return;
        nativeTouch(servoWebTag, nativeViewId, MotionEvent.ACTION_DOWN, 0, x, y);
        nativeTouch(servoWebTag, nativeViewId, MotionEvent.ACTION_UP, 0, x, y);
    }

    @Override
    public void scrollByPixels(int dx, int dy) {
        if (bound()) nativeWheel(servoWebTag, nativeViewId, dx, dy);
    }

    @Override
    public void pause() {
        paused = true;
        if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
        frameScheduled = false;
        if (bound()) nativeSetThrottled(servoWebTag, nativeViewId, true);
    }

    @Override
    public void resume() {
        paused = false;
        if (bound()) nativeSetThrottled(servoWebTag, nativeViewId, false);
        scheduleFrame();
    }

    /** Capture what is on screen, native overlays included, like the Chromium host. */
    public void captureScreenshot(final long requestId) {
        runOnMainThread(() -> {
            int width = getWidth();
            int height = getHeight();
            if (width <= 0 || height <= 0) {
                nativeScreenshotResult(requestId, new byte[0], "WebView has zero size; cannot capture");
                return;
            }
            Activity activity = findActivity(getContext());
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
                    && activity != null && activity.getWindow() != null) {
                Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
                int[] location = new int[2];
                getLocationInWindow(location);
                Rect source = new Rect(location[0], location[1],
                        location[0] + width, location[1] + height);
                try {
                    PixelCopy.request(activity.getWindow(), source, bitmap, result -> {
                        if (result == PixelCopy.SUCCESS) {
                            deliverScreenshot(requestId, bitmap);
                        } else {
                            bitmap.recycle();
                            captureSurfaceBitmap(requestId);
                        }
                    }, new Handler(Looper.getMainLooper()));
                    return;
                } catch (Throwable error) {
                    bitmap.recycle();
                    android.util.Log.w(TAG, "PixelCopy screenshot threw", error);
                }
            }
            captureSurfaceBitmap(requestId);
        });
    }

    private void captureSurfaceBitmap(long requestId) {
        Bitmap bitmap = servoSurface.isAvailable() ? servoSurface.getBitmap() : null;
        if (bitmap == null) {
            nativeScreenshotResult(requestId, new byte[0], "Servo surface is not ready");
            return;
        }
        deliverScreenshot(requestId, bitmap);
    }

    private void deliverScreenshot(long requestId, Bitmap bitmap) {
        ByteArrayOutputStream stream = new ByteArrayOutputStream(
                Math.max(64 * 1024, bitmap.getWidth() * bitmap.getHeight() / 4));
        boolean ok = bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream);
        bitmap.recycle();
        nativeScreenshotResult(requestId, ok ? stream.toByteArray() : new byte[0],
                ok ? "" : "Bitmap.compress(PNG) returned false");
    }

    private static Activity findActivity(Context context) {
        Context current = context;
        while (current instanceof ContextWrapper) {
            if (current instanceof Activity) return (Activity) current;
            current = ((ContextWrapper) current).getBaseContext();
        }
        return null;
    }

    @Override
    public void destroy() {
        runOnMainThread(this::destroyOnMainThread);
    }

    private void destroyOnMainThread() {
        destroyed = true;
        if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
        releaseNativeSurface(true);
        if (servoWebTag != null) {
            LingXiaWebView.cancelServoEvaluations(servoWebTag);
            WeakReference<LingXiaServoView> reference = sViews.get(servoWebTag);
            if (reference != null && reference.get() == this) sViews.remove(servoWebTag, reference);
        }
        touchInterceptor = null;
        NativeComponentMessageHandler handler = nativeComponentMessageHandler;
        nativeComponentMessageHandler = null;
        pendingComponentMessages.clear();
        if (handler != null) handler.onDestroyed();
        EmbedderControlHandler controlHandler = embedderControlHandler;
        embedderControlHandler = null;
        if (controlHandler != null) controlHandler.onDestroyed();
        ViewGroup parent = getParent() instanceof ViewGroup ? (ViewGroup) getParent() : null;
        if (parent != null) {
            parent.removeView(this);
            if (parent.getChildCount() == 0 && parent.getParent() instanceof ViewGroup) {
                ((ViewGroup) parent.getParent()).removeView(parent);
            }
        }
    }

    public void setNativeComponentMessageHandler(NativeComponentMessageHandler handler) {
        runOnMainThread(() -> {
            nativeComponentMessageHandler = handler;
            if (handler == null) {
                pendingComponentMessages.clear();
                return;
            }
            while (!pendingComponentMessages.isEmpty()) {
                handler.onMessage(pendingComponentMessages.removeFirst());
            }
        });
    }

    public void setEmbedderControlHandler(EmbedderControlHandler handler) {
        runOnMainThread(() -> {
            EmbedderControlHandler previous = embedderControlHandler;
            embedderControlHandler = handler;
            if (previous != null && previous != handler) previous.onDestroyed();
        });
    }

    public void completeEmbedderControl(long requestId, boolean confirm, String value) {
        if (!bound()) return;
        boolean queued = nativeCompleteEmbedderControl(
                servoWebTag,
                nativeViewId,
                requestId,
                confirm ? "confirm" : "cancel",
                value != null ? value : "");
        if (!queued) {
            android.util.Log.w(TAG, "Dropped embedder control response request=" + requestId);
        }
    }

    static void showEmbedderControl(
            final String webTag,
            final long nativeViewId,
            final long requestId,
            final String kind,
            final String payload) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view == null) return;
            EmbedderControlHandler handler = view.embedderControlHandler;
            if (handler != null) handler.show(requestId, kind, payload);
            else view.completeEmbedderControl(requestId, false, "");
        });
    }

    static void hideEmbedderControl(
            final String webTag, final long nativeViewId, final long requestId) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view != null && view.embedderControlHandler != null) {
                view.embedderControlHandler.hide(requestId);
            }
        });
    }

    static void dispatchNativeComponentMessage(
            final String webTag, final long nativeViewId, final String message) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view == null || !view.strictSecurityProfile) return;
            NativeComponentMessageHandler handler = view.nativeComponentMessageHandler;
            if (handler != null) {
                handler.onMessage(message);
                return;
            }
            if (view.pendingComponentMessages.size() == MAX_PENDING_COMPONENT_MESSAGES) {
                view.pendingComponentMessages.removeFirst();
            }
            view.pendingComponentMessages.addLast(message);
        });
    }

    static void dispatchScroll(final String webTag, final long nativeViewId, final String message) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view == null) return;
            try {
                JSONObject scroll = new JSONObject(message);
                double dpr = scroll.optDouble("dpr", 1.0);
                if (!(dpr > 0)) dpr = 1.0;
                view.contentScrollX = (int) Math.round(scroll.optDouble("x", 0) * dpr);
                view.contentScrollY = (int) Math.round(scroll.optDouble("y", 0) * dpr);
                // Overlay sync runs in pre-draw listeners on this view.
                view.invalidate();
            } catch (Exception error) {
                android.util.Log.w(TAG, "Dropped malformed Servo scroll report", error);
            }
        });
    }

    static void showInputMethod(
            final String webTag,
            final long nativeViewId,
            final int type,
            final String text,
            final int insertionPoint,
            final boolean multiline,
            final boolean allowVirtualKeyboard) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view == null) return;
            view.editorInputType = androidInputType(type, multiline);
            view.editorImeOptions = multiline
                    ? EditorInfo.IME_FLAG_NO_ENTER_ACTION
                    : EditorInfo.IME_ACTION_DONE;
            view.editable.replace(0, view.editable.length(), text != null ? text : "");
            int cursor = insertionPoint >= 0
                    ? Math.min(insertionPoint, view.editable.length())
                    : view.editable.length();
            Selection.setSelection(view.editable, cursor);
            view.composing = false;
            view.composingText = "";
            view.requestFocus();
            InputMethodManager manager = (InputMethodManager) view.getContext()
                    .getSystemService(Context.INPUT_METHOD_SERVICE);
            if (manager == null) return;
            manager.restartInput(view);
            if (allowVirtualKeyboard) manager.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
        });
    }

    static void hideInputMethod(final String webTag, final long nativeViewId) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag, nativeViewId);
            if (view == null) return;
            if (view.composing) view.finishComposition();
            InputMethodManager manager = (InputMethodManager) view.getContext()
                    .getSystemService(Context.INPUT_METHOD_SERVICE);
            if (manager != null) manager.hideSoftInputFromWindow(view.getWindowToken(), 0);
        });
    }

    private static LingXiaServoView findView(String webTag, long nativeViewId) {
        WeakReference<LingXiaServoView> reference = sViews.get(webTag);
        LingXiaServoView view = reference != null ? reference.get() : null;
        if (view == null && reference != null) sViews.remove(webTag, reference);
        return view != null && view.nativeViewId == nativeViewId ? view : null;
    }

    private static void runOnMainThread(Runnable action) {
        if (Looper.myLooper() == Looper.getMainLooper()) action.run();
        else new Handler(Looper.getMainLooper()).post(action);
    }

    private static int androidInputType(int type, boolean multiline) {
        int value;
        switch (type) {
            case 1:
                value = InputType.TYPE_CLASS_DATETIME | InputType.TYPE_DATETIME_VARIATION_DATE;
                break;
            case 2:
                value = InputType.TYPE_CLASS_DATETIME;
                break;
            case 3:
                value = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;
                break;
            case 4:
            case 5:
            case 12:
                value = InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL;
                break;
            case 6:
                value = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
                break;
            case 7:
                value = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_FILTER;
                break;
            case 8:
                value = InputType.TYPE_CLASS_PHONE;
                break;
            case 10:
                value = InputType.TYPE_CLASS_DATETIME | InputType.TYPE_DATETIME_VARIATION_TIME;
                break;
            case 11:
                value = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI;
                break;
            default:
                value = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_NORMAL;
                break;
        }
        if (multiline) value |= InputType.TYPE_TEXT_FLAG_MULTI_LINE;
        return value;
    }

    private boolean forwardKeyEvent(KeyEvent event) {
        if (event.getAction() != KeyEvent.ACTION_DOWN && event.getAction() != KeyEvent.ACTION_UP) {
            return false;
        }
        if (!isSupportedKey(event)) return false;
        nativeKey(
                servoWebTag,
                nativeViewId,
                event.getAction(),
                event.getKeyCode(),
                event.getUnicodeChar(),
                event.getMetaState(),
                event.getRepeatCount());
        return true;
    }

    private static boolean isSupportedKey(KeyEvent event) {
        if (event.getUnicodeChar() != 0) return true;
        switch (event.getKeyCode()) {
            case KeyEvent.KEYCODE_DPAD_UP:
            case KeyEvent.KEYCODE_DPAD_DOWN:
            case KeyEvent.KEYCODE_DPAD_LEFT:
            case KeyEvent.KEYCODE_DPAD_RIGHT:
            case KeyEvent.KEYCODE_TAB:
            case KeyEvent.KEYCODE_ENTER:
            case KeyEvent.KEYCODE_DEL:
            case KeyEvent.KEYCODE_PAGE_UP:
            case KeyEvent.KEYCODE_PAGE_DOWN:
            case KeyEvent.KEYCODE_ESCAPE:
            case KeyEvent.KEYCODE_FORWARD_DEL:
            case KeyEvent.KEYCODE_MOVE_HOME:
            case KeyEvent.KEYCODE_MOVE_END:
                return true;
            default:
                return false;
        }
    }

    private void startComposition() {
        if (composing || !bound()) return;
        nativeIme(servoWebTag, nativeViewId, 0, "");
        composing = true;
    }

    private void finishComposition() {
        if (!composing || !bound()) return;
        nativeIme(servoWebTag, nativeViewId, 2, composingText);
        composing = false;
        composingText = "";
    }

    private final class ServoInputConnection extends BaseInputConnection {
        ServoInputConnection() {
            super(LingXiaServoView.this, true);
        }

        @Override
        public Editable getEditable() {
            return editable;
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition) {
            startComposition();
            composingText = text != null ? text.toString() : "";
            if (bound()) nativeIme(servoWebTag, nativeViewId, 1, composingText);
            return super.setComposingText(text, newCursorPosition);
        }

        @Override
        public boolean commitText(CharSequence text, int newCursorPosition) {
            String committed = text != null ? text.toString() : "";
            if (!composing) startComposition();
            composingText = committed;
            finishComposition();
            return super.commitText(text, newCursorPosition);
        }

        @Override
        public boolean finishComposingText() {
            finishComposition();
            return super.finishComposingText();
        }

        @Override
        public boolean deleteSurroundingText(int beforeLength, int afterLength) {
            if (composing) finishComposition();
            for (int index = 0; index < beforeLength; index++) {
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL));
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_DEL));
            }
            for (int index = 0; index < afterLength; index++) {
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_FORWARD_DEL));
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_FORWARD_DEL));
            }
            return super.deleteSurroundingText(beforeLength, afterLength);
        }

        @Override
        public boolean sendKeyEvent(KeyEvent event) {
            return forwardKeyEvent(event);
        }

        @Override
        public boolean performEditorAction(int actionCode) {
            if (actionCode == EditorInfo.IME_ACTION_DONE || actionCode == EditorInfo.IME_ACTION_GO
                    || actionCode == EditorInfo.IME_ACTION_NEXT
                    || actionCode == EditorInfo.IME_ACTION_SEARCH
                    || actionCode == EditorInfo.IME_ACTION_SEND) {
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER));
                forwardKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER));
                return true;
            }
            return super.performEditorAction(actionCode);
        }
    }

    private native void nativeSurfaceCreated(
            String webTag, long nativeViewId, Surface surface, int width, int height, float density);
    private native void nativeSurfaceChanged(String webTag, long nativeViewId, int width, int height);
    private native boolean nativeSurfaceDestroyed(String webTag, long nativeViewId, long releaseToken);
    private native boolean nativeCompleteEmbedderControl(
            String webTag, long nativeViewId, long requestId, String action, String value);
    private native void nativeFrame(String webTag, long nativeViewId);
    private native void nativeSetThrottled(String webTag, long nativeViewId, boolean throttled);
    private native void nativeSetSurfaceShown(String webTag, long nativeViewId, boolean shown);
    private native void nativeTouch(
            String webTag, long nativeViewId, int action, int pointerId, float x, float y);
    private native void nativeWheel(String webTag, long nativeViewId, double dx, double dy);
    private native void nativeIme(String webTag, long nativeViewId, int state, String text);
    private native void nativeKey(
            String webTag,
            long nativeViewId,
            int action,
            int keyCode,
            int unicodeCodePoint,
            int metaState,
            int repeatCount);
    private native String nativeGetUrl(String webTag, long nativeViewId);
    private native String nativeGetTitle(String webTag, long nativeViewId);
    private native boolean nativeCanGoBack(String webTag, long nativeViewId);
    private native boolean nativeCanGoForward(String webTag, long nativeViewId);
    private native void nativeNavigate(String webTag, long nativeViewId, int action);
    private native void nativeEvaluate(
            String webTag, long nativeViewId, long requestId, String script);
    private native void nativeScreenshotResult(long requestId, byte[] pngBytes, String error);
}
