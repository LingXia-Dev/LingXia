package com.lingxia.webview;

import android.content.Context;
import android.graphics.Color;
import android.graphics.SurfaceTexture;
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
import java.lang.ref.WeakReference;
import java.util.ArrayDeque;
import java.util.concurrent.ConcurrentHashMap;

/** Android view host for Servo's Rust embedding API. */
public final class LingXiaServoView extends FrameLayout implements LingXiaWebViewHost,
        TextureView.SurfaceTextureListener, Choreographer.FrameCallback {
    public interface NativeComponentMessageHandler {
        void onMessage(String message);
        void onDestroyed();
    }

    private static final int MAX_PENDING_COMPONENT_MESSAGES = 128;
    private static final String TAG = "LingXiaServoView";
    private static final ConcurrentHashMap<String, WeakReference<LingXiaServoView>> sViews =
            new ConcurrentHashMap<>();
    private final TextureView servoSurface;
    private final Editable editable = new SpannableStringBuilder();
    private final ServoInputConnection inputConnection;
    private Surface nativeSurface;
    private String servoWebTag;
    private String appId;
    private String currentPath;
    private long sessionId;
    private boolean strictSecurityProfile = true;
    private boolean attached;
    private boolean frameScheduled;
    private boolean paused;
    private boolean composing;
    private String composingText = "";
    private int editorInputType = InputType.TYPE_CLASS_TEXT;
    private int editorImeOptions = EditorInfo.IME_ACTION_DONE;
    private final ArrayDeque<String> pendingComponentMessages = new ArrayDeque<>();
    private NativeComponentMessageHandler nativeComponentMessageHandler;

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

    void initialize(String appId, String path, long sessionId, boolean strictSecurityProfile) {
        this.appId = appId;
        this.currentPath = path;
        this.sessionId = sessionId;
        this.strictSecurityProfile = strictSecurityProfile;
        servoWebTag = appId + ":" + path + (sessionId > 0 ? "#" + sessionId : "");
        sViews.put(servoWebTag, new WeakReference<>(this));
        SurfaceTexture texture = servoSurface.getSurfaceTexture();
        android.util.Log.d(TAG, "bind tag=" + servoWebTag + " valid="
                + (servoSurface.isAvailable() && texture != null) + " size="
                + servoSurface.getWidth() + "x" + servoSurface.getHeight());
        if (servoSurface.isAvailable() && texture != null
                && servoSurface.getWidth() > 0 && servoSurface.getHeight() > 0) {
            attachNativeSurface(texture, servoSurface.getWidth(), servoSurface.getHeight());
        }
    }

    private void attachNativeSurface(SurfaceTexture texture, int width, int height) {
        if (nativeSurface != null) nativeSurface.release();
        nativeSurface = new Surface(texture);
        createNativeSurface(nativeSurface, width, height);
    }

    private float density() {
        DisplayMetrics metrics = getResources().getDisplayMetrics();
        return metrics != null ? metrics.density : 1.0f;
    }

    private void createNativeSurface(Surface surface, int width, int height) {
        if (servoWebTag == null || attached) return;
        android.util.Log.d(TAG, "create native surface tag=" + servoWebTag + " size="
                + width + "x" + height + " valid=" + surface.isValid());
        nativeSurfaceCreated(servoWebTag, surface, width, height, density());
        attached = true;
        scheduleFrame();
    }

    private void scheduleFrame() {
        if (!frameScheduled && attached && !paused) {
            frameScheduled = true;
            Choreographer.getInstance().postFrameCallback(this);
        }
    }

    @Override
    public void onSurfaceTextureAvailable(SurfaceTexture texture, int width, int height) {
        android.util.Log.d(TAG, "surface available tag=" + servoWebTag + " size="
                + width + "x" + height);
        attachNativeSurface(texture, width, height);
    }

    @Override
    public void onSurfaceTextureSizeChanged(SurfaceTexture texture, int width, int height) {
        if (servoWebTag != null && attached) nativeSurfaceChanged(servoWebTag, width, height);
    }

    @Override
    public boolean onSurfaceTextureDestroyed(SurfaceTexture texture) {
        android.util.Log.d(TAG, "surface destroyed tag=" + servoWebTag);
        if (servoWebTag != null && attached) nativeSurfaceDestroyed(servoWebTag);
        attached = false;
        frameScheduled = false;
        if (nativeSurface != null) {
            nativeSurface.release();
            nativeSurface = null;
        }
        return true;
    }

    @Override
    public void onSurfaceTextureUpdated(SurfaceTexture texture) {}

    @Override
    public void doFrame(long frameTimeNanos) {
        frameScheduled = false;
        if (servoWebTag != null && attached) nativeFrame(servoWebTag);
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
        return servoWebTag != null ? nativeGetUrl(servoWebTag) : "";
    }

    @Override
    public String getTitle() {
        return servoWebTag != null ? nativeGetTitle(servoWebTag) : "";
    }

    @Override
    public boolean canGoBack() {
        return servoWebTag != null && nativeCanGoBack(servoWebTag);
    }

    @Override
    public boolean canGoForward() {
        return servoWebTag != null && nativeCanGoForward(servoWebTag);
    }

    @Override
    public boolean usesStrictSecurityProfile() {
        return strictSecurityProfile;
    }

    @Override
    public boolean retainsSurfaceWhenHidden() {
        return true;
    }

    @Override
    public void reload() {
        if (servoWebTag != null) nativeNavigate(servoWebTag, 0);
    }

    @Override
    public void goBack() {
        if (servoWebTag != null) nativeNavigate(servoWebTag, 1);
    }

    @Override
    public void goForward() {
        if (servoWebTag != null) nativeNavigate(servoWebTag, 2);
    }

    @Override
    public void evaluateJavascript(String script, ValueCallback<String> callback) {
        if (servoWebTag == null) {
            if (callback != null) callback.onReceiveValue("null");
            return;
        }
        long requestId = LingXiaWebView.registerServoEvaluation(servoWebTag, callback);
        nativeEvaluate(servoWebTag, requestId, script);
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        if (servoWebTag == null) return false;
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) requestFocus();
        int index = event.getActionIndex();
        nativeTouch(
                servoWebTag,
                event.getActionMasked(),
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
        if (servoWebTag != null && forwardKeyEvent(event)) return true;
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void dispatchClickAt(float x, float y) {
        if (servoWebTag == null) return;
        nativeTouch(servoWebTag, MotionEvent.ACTION_DOWN, 0, x, y);
        nativeTouch(servoWebTag, MotionEvent.ACTION_UP, 0, x, y);
    }

    @Override
    public void scrollByPixels(int dx, int dy) {
        if (servoWebTag != null) nativeWheel(servoWebTag, dx, dy);
    }

    @Override
    public void pause() {
        paused = true;
        if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
        frameScheduled = false;
    }

    @Override
    public void resume() {
        paused = false;
        scheduleFrame();
    }

    @Override
    public void destroy() {
        runOnMainThread(this::destroyOnMainThread);
    }

    private void destroyOnMainThread() {
        android.util.Log.d(TAG, "destroy tag=" + servoWebTag + " attached=" + attached);
        if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
        if (attached && servoWebTag != null) nativeSurfaceDestroyed(servoWebTag);
        if (servoWebTag != null) {
            LingXiaWebView.cancelServoEvaluations(servoWebTag);
            WeakReference<LingXiaServoView> reference = sViews.get(servoWebTag);
            if (reference != null && reference.get() == this) sViews.remove(servoWebTag, reference);
        }
        attached = false;
        frameScheduled = false;
        servoSurface.setSurfaceTextureListener(null);
        if (nativeSurface != null) {
            nativeSurface.release();
            nativeSurface = null;
        }
        NativeComponentMessageHandler handler = nativeComponentMessageHandler;
        nativeComponentMessageHandler = null;
        pendingComponentMessages.clear();
        if (handler != null) handler.onDestroyed();
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

    static void dispatchNativeComponentMessage(final String webTag, final String message) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag);
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

    static void showInputMethod(
            final String webTag,
            final int type,
            final String text,
            final int insertionPoint,
            final boolean multiline,
            final boolean allowVirtualKeyboard) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag);
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

    static void hideInputMethod(final String webTag) {
        runOnMainThread(() -> {
            LingXiaServoView view = findView(webTag);
            if (view == null) return;
            if (view.composing) view.finishComposition();
            InputMethodManager manager = (InputMethodManager) view.getContext()
                    .getSystemService(Context.INPUT_METHOD_SERVICE);
            if (manager != null) manager.hideSoftInputFromWindow(view.getWindowToken(), 0);
        });
    }

    private static LingXiaServoView findView(String webTag) {
        WeakReference<LingXiaServoView> reference = sViews.get(webTag);
        LingXiaServoView view = reference != null ? reference.get() : null;
        if (view == null && reference != null) sViews.remove(webTag, reference);
        return view;
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
        if (composing || servoWebTag == null) return;
        nativeIme(servoWebTag, 0, "");
        composing = true;
    }

    private void finishComposition() {
        if (!composing || servoWebTag == null) return;
        nativeIme(servoWebTag, 2, composingText);
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
            if (servoWebTag != null) nativeIme(servoWebTag, 1, composingText);
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
            String webTag, Surface surface, int width, int height, float density);
    private native void nativeSurfaceChanged(String webTag, int width, int height);
    private native void nativeSurfaceDestroyed(String webTag);
    private native void nativeFrame(String webTag);
    private native void nativeTouch(String webTag, int action, int pointerId, float x, float y);
    private native void nativeWheel(String webTag, double dx, double dy);
    private native void nativeIme(String webTag, int state, String text);
    private native void nativeKey(
            String webTag, int action, int keyCode, int unicodeCodePoint, int metaState, int repeatCount);
    private native String nativeGetUrl(String webTag);
    private native String nativeGetTitle(String webTag);
    private native boolean nativeCanGoBack(String webTag);
    private native boolean nativeCanGoForward(String webTag);
    private native void nativeNavigate(String webTag, int action);
    private native void nativeEvaluate(String webTag, long requestId, String script);
}
