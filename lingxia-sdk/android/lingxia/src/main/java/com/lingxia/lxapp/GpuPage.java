package com.lingxia.lxapp;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.widget.FrameLayout;
import com.lingxia.app.Lingxia;
import com.lingxia.ui.GpuView;
import java.util.HashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** SDK-owned page shell. Registry and worker ownership are confined to the main thread. */
public final class GpuPage extends FrameLayout {
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final HashMap<Long, GpuPage> PAGES = new HashMap<>();
    private static final HashMap<String, Worker> WORKERS = new HashMap<>();
    private static final class Worker {
        final ExecutorService executor = Executors.newSingleThreadExecutor(r -> new Thread(r, "lingxia-gpu-view"));
        int pages;
    }
    private static final ExecutorService IMAGE_IO = Executors.newFixedThreadPool(2, r -> new Thread(r, "lingxia-gpu-images"));
    private static native long beginImage();
    private static native void cancelImage(long request);
    private static native byte[] readImage(long token, long request, String source);
    private static native void created(long token, GpuPage page, String error);
    private static native void presented(long token, long generation);
    private static native boolean restart(long token, long generation);
    private static native void message(long token, long generation, String frame);
    private final String appId, path, webTag, workerKey;
    private final GpuView content;
    private boolean reported;
    private boolean frameReported;

    private GpuPage(Context context, long token, String appId, long session, String path, String webTag, String source, boolean logic) {
        super(context);
        this.appId = appId; this.path = path; this.webTag = webTag;
        workerKey = appId + ":" + session;
        Worker worker = WORKERS.get(workerKey);
        if (worker == null) { worker = new Worker(); WORKERS.put(workerKey, worker); }
        worker.pages++;
        GpuView candidate = null;
        try {
            candidate = new GpuView(context, source, worker.executor, new GpuView.Listener() {
                public Runnable onImageRequest(String source, java.util.function.Consumer<byte[]> result) {
                    long request = beginImage();
                    java.util.concurrent.Future<?> work = IMAGE_IO.submit(() -> {
                        byte[] bytes = null;
                        try { bytes = readImage(token, request, source); }
                        catch (Throwable error) { android.util.Log.w("LingXia.UI", "Image resource failed", error); }
                        finally { cancelImage(request); }
                        result.accept(bytes);
                    });
                    return () -> { cancelImage(request); work.cancel(false); };
                }
                public void onLoaded() {
                    if (!reported) { reported = true; created(token, GpuPage.this, null); }
                }
                public void onFrame(String metadata) {
                    if (!frameReported) { frameReported = true; presented(token, content.getConnectionGeneration()); }
                }
                public void onMessage(long generation, String frame) { message(token, generation, frame); }
                public boolean onRestart(long generation) { return restart(token, generation); }
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
            releaseWorker(workerKey);
            throw error;
        }
    }

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
    public void capturePreview(java.util.function.Consumer<android.graphics.Bitmap> completed) { content.captureFrame(completed); }
    public void captureReadyPreview(java.util.function.Consumer<android.graphics.Bitmap> completed) { content.captureFrameWhenReady(completed); }
    public void afterNextPresentation(java.util.function.Consumer<Boolean> completed) { content.afterNextPresentation(completed); }

    public static void create(long token, String appId, long session, String path, String webTag, String source, boolean logic) {
        MAIN.post(() -> {
            try {
                Context context = Lingxia.applicationContext();
                if (context == null) throw new IllegalStateException("SDK context unavailable");
                PAGES.put(token, new GpuPage(context, token, appId, session, path, webTag, source, logic));
            } catch (Throwable error) { created(token, null, error.toString()); }
        });
    }

    public static void connect(long token, long generation, String nonce) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) { page.frameReported = false; page.content.connect(generation, nonce); } });
    }
    public static void requestLeave(long token, long generation, String reason) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.requestLeave(generation, reason); });
    }

    public static void receive(long token, long generation, String frame) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.receive(generation, frame); });
    }

    public static void updateEnvironment(long token, String update) {
        MAIN.post(() -> { GpuPage page = PAGES.get(token); if (page != null) page.content.updateEnvironment(update); });
    }

    public static void reload(long token, String source) {
        MAIN.post(() -> {
            GpuPage page = PAGES.get(token);
            if (page != null) { page.frameReported = false; page.content.reload(source); }
        });
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
                releaseWorker(page.workerKey);
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

    private static void releaseWorker(String key) {
        Worker worker = WORKERS.get(key);
        if (--worker.pages == 0) {
            WORKERS.remove(key);
            // dispose enqueues context release before shutdown; shutdown drains that work.
            worker.executor.shutdown();
        }
    }
}
