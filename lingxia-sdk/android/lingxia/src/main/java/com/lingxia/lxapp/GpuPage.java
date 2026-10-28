package com.lingxia.lxapp;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.widget.FrameLayout;
import com.lingxia.app.Lingxia;
import com.lingxia.ui.GpuView;
import java.util.HashMap;

/** SDK-owned page shell. The registry is confined to the main thread. */
public final class GpuPage extends FrameLayout {
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final HashMap<Long, GpuPage> PAGES = new HashMap<>();
    private static native void created(long token, GpuPage page, String error);
    /** A presented frame; true once the page took it as its first, so reports stop until the next connection. */
    private static native boolean presented(long token, long generation);
    private static native boolean restart(long token, long generation);
    private static native void message(long token, long generation, String frame);
    private static native void automationReply(long replyId, String reply);
    private static final String INACTIVE = "{\"error\":\"page is not active: the GPU page was disposed\"}";
    private final String appId, path, webTag;
    private final GpuView content;
    private boolean reported;
    private boolean frameReported, frameWaitLogged;

    private GpuPage(Context context, long token, String appId, String worker, String path, String webTag, String source, boolean logic) {
        super(context);
        this.appId = appId; this.path = path; this.webTag = webTag;
        GpuView candidate = null;
        try {
            candidate = new GpuView(context, source, worker, token, new GpuView.Listener() {
                public void onLoaded() {
                    if (!reported) { reported = true; created(token, GpuPage.this, null); }
                }
                public void onFrame(String metadata) {
                    if (frameReported) return;
                    frameReported = presented(token, content.getConnectionGeneration());
                    if (!frameReported && !frameWaitLogged) {
                        frameWaitLogged = true;
                        android.util.Log.d("LingXia.Nav", "first frame of " + webTag + " not its current view's; reporting the next");
                    }
                }
                public void onMessage(long generation, String frame) { message(token, generation, frame); }
                public boolean onRestart(long generation) { return restart(token, generation); }
                public void onChrome(GpuView.Chrome chrome) {
                    if (chromeListener != null) chromeListener.accept(GpuPage.this, chrome);
                }
                public void onError(Throwable error) {
                    if (!reported) { reported = true; created(token, null, error.toString()); }
                    android.util.Log.e("LingXia.UI", "GPU page " + webTag, error);
                }
            }, logic);
            candidate.pause();
            addView(candidate, new FrameLayout.LayoutParams(-1, -1));
            content = candidate;
        } catch (Throwable error) {
            if (candidate != null) candidate.dispose();
            throw error;
        }
    }

    private java.util.function.BiConsumer<GpuPage, GpuView.Chrome> chromeListener;
    /** Whoever shows this page applies its chrome; it may arrive before the page is current. */
    public void setChromeListener(java.util.function.BiConsumer<GpuPage, GpuView.Chrome> listener) { chromeListener = listener; }
    /** The chrome this page last asked for, or null before its first frame. */
    public GpuView.Chrome getChrome() { return content.getChrome(); }

    public String getAppId() { return appId; }
    public String getCurrentPath() { return path; }
    public String getWebTag() { return webTag; }
    public void setChromeInsets(int left, int top, int right, int bottom) {
        content.setChromeInsets(left, top, right, bottom);
    }
    public void setPullToRefresh(GpuView.PullToRefresh callback) { content.setPullToRefresh(callback); }
    public void clearPullToRefresh(GpuView.PullToRefresh callback) { content.clearPullToRefresh(callback); }
    @Override public boolean canScrollVertically(int direction) { return content.canScrollVertically(direction); }
    public void resume() { content.resume(); }
    public void pause() { content.pause(); }
    public void freeze() { content.freeze(); }
    public void capturePreview(java.util.function.Consumer<android.graphics.Bitmap> completed) { content.captureFrame(completed); }
    public void captureReadyPreview(java.util.function.Consumer<android.graphics.Bitmap> completed) { content.captureFrameWhenReady(completed); }
    public void afterNextPresentation(java.util.function.Consumer<Boolean> completed) { content.afterNextPresentation(completed); }
    public void revealOnFirstFrame(Runnable then) { content.revealOnFirstFrame(then); }
    public boolean hasLiveFrame() { return content.hasLiveFrame(); }

    /** {@code worker} keys the View worker; pages of one app session share it. */
    public static void create(long token, String appId, String worker, String path, String webTag, String source, boolean logic) {
        MAIN.post(() -> {
            try {
                Context context = Lingxia.applicationContext();
                if (context == null) throw new IllegalStateException("SDK context unavailable");
                PAGES.put(token, new GpuPage(context, token, appId, worker, path, webTag, source, logic));
            } catch (Throwable error) { created(token, null, error.toString()); }
        });
    }

    public static void connect(long token, long generation, String nonce) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) { page.frameReported = false; page.frameWaitLogged = false; page.content.connect(generation, nonce); } });
    }
    public static void requestLeave(long token, long generation, String reason) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.requestLeave(generation, reason); });
    }

    public static void receive(long token, long generation, String frame) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.receive(generation, frame); });
    }

    /** Automation runs on the main thread, after the frames already posted there. */
    public static void automate(long token, String request, long replyId) {
        MAIN.post(() -> {
            GpuPage page = PAGES.get(token);
            if (page == null) { automationReply(replyId, INACTIVE); return; }
            String op;
            try { op = new org.json.JSONObject(request).optString("op"); }
            catch (org.json.JSONException error) { automationReply(replyId, "{\"error\":\"E_UNSUPPORTED: malformed automation request\"}"); return; }
            if (!"screenshot".equals(op)) { automationReply(replyId, page.content.automate(request)); return; }
            page.content.captureFrameWhenReady(frame -> {
                if (frame == null) { automationReply(replyId, "{\"error\":\"E_PAGE_NOT_READY: no presented frame\"}"); return; }
                java.io.ByteArrayOutputStream png = new java.io.ByteArrayOutputStream();
                frame.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, png);
                frame.recycle();
                try {
                    org.json.JSONObject value = new org.json.JSONObject()
                        .put("png", android.util.Base64.encodeToString(png.toByteArray(), android.util.Base64.NO_WRAP));
                    automationReply(replyId, new org.json.JSONObject().put("value", value).toString());
                } catch (org.json.JSONException error) { throw new IllegalStateException(error); }
            });
        });
    }

    public static void retainSurface(long token, boolean retained) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.setSurfaceRetained(retained); });
    }

    public static void updateEnvironment(long token, String update) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.updateEnvironment(update); });
    }

    public static void reload(long token, String source) {
        MAIN.post(() -> {
            GpuPage page = PAGES.get(token);
            if (page != null) { page.frameReported = false; page.frameWaitLogged = false; page.content.reload(source); }
        });
    }

    /** Dev hot-swap: the page keeps its state and its presented frame. */
    public static void swapModule(long token, String source) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.swapModule(source); });
    }

    public static void park(long token) {
        MAIN.post(() -> {
            GpuPage page = PAGES.get(token);
            if (page != null) page.preserveBeforeRetirement(() -> page.content.park());
        });
    }

    public static void dispose(long token) {
        MAIN.post(() -> {
            GpuPage page = PAGES.remove(token);
            if (page == null) return;
            page.reported = true;
            Runnable release = () -> {
                page.content.dispose();
                if (page.getParent() instanceof android.view.ViewGroup) {
                    ((android.view.ViewGroup)page.getParent()).removeView(page);
                }
            };
            page.preserveBeforeRetirement(release);
        });
    }

    private void preserveBeforeRetirement(Runnable release) {
        android.view.View parent = getParent() instanceof android.view.View
                ? (android.view.View)getParent() : null;
        if (parent != null && parent.getContext() instanceof LxAppActivity)
            ((LxAppActivity)parent.getContext()).preserveGpuPageBeforeDisposal(this, release);
        else release.run();
    }
}
