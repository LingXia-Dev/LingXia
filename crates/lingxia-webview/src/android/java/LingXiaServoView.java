package com.lingxia.webview;

import android.content.Context;
import android.graphics.Color;
import android.graphics.SurfaceTexture;
import android.util.DisplayMetrics;
import android.view.Choreographer;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.TextureView;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.ValueCallback;
import android.widget.FrameLayout;

/** Android view host for Servo's Rust embedding API. */
public final class LingXiaServoView extends FrameLayout implements LingXiaWebViewHost,
        TextureView.SurfaceTextureListener, Choreographer.FrameCallback {
    private static final String TAG = "LingXiaServoView";
    private final TextureView servoSurface;
    private Surface nativeSurface;
    private String servoWebTag;
    private String appId;
    private String currentPath;
    private long sessionId;
    private boolean attached;
    private boolean frameScheduled;
    private boolean paused;

    public LingXiaServoView(Context context) {
        super(context);
        setBackgroundColor(Color.TRANSPARENT);
        servoSurface = new TextureView(context);
        servoSurface.setSurfaceTextureListener(this);
        servoSurface.setFocusable(true);
        servoSurface.setFocusableInTouchMode(true);
        addView(servoSurface, new ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
    }

    void initialize(String appId, String path, long sessionId) {
        this.appId = appId;
        this.currentPath = path;
        this.sessionId = sessionId;
        servoWebTag = appId + ":" + path + (sessionId > 0 ? "#" + sessionId : "");
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
        if (servoWebTag != null) nativeEvaluate(servoWebTag, script);
        if (callback != null) callback.onReceiveValue("null");
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        if (servoWebTag == null) return false;
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
        android.util.Log.d(TAG, "destroy tag=" + servoWebTag + " attached=" + attached);
        if (frameScheduled) Choreographer.getInstance().removeFrameCallback(this);
        if (attached && servoWebTag != null) nativeSurfaceDestroyed(servoWebTag);
        attached = false;
        frameScheduled = false;
        servoSurface.setSurfaceTextureListener(null);
        if (nativeSurface != null) {
            nativeSurface.release();
            nativeSurface = null;
        }
    }

    private native void nativeSurfaceCreated(
            String webTag, Surface surface, int width, int height, float density);
    private native void nativeSurfaceChanged(String webTag, int width, int height);
    private native void nativeSurfaceDestroyed(String webTag);
    private native void nativeFrame(String webTag);
    private native void nativeTouch(String webTag, int action, int pointerId, float x, float y);
    private native void nativeWheel(String webTag, double dx, double dy);
    private native String nativeGetUrl(String webTag);
    private native String nativeGetTitle(String webTag);
    private native boolean nativeCanGoBack(String webTag);
    private native boolean nativeCanGoForward(String webTag);
    private native void nativeNavigate(String webTag, int action);
    private native void nativeEvaluate(String webTag, String script);
}
