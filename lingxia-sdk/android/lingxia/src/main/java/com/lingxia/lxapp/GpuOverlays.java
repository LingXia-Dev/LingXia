package com.lingxia.lxapp;

import android.app.Activity;
import android.content.Context;
import android.graphics.PixelFormat;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import com.lingxia.app.Lingxia;
import com.lingxia.view.GpuView;
import java.util.ArrayList;
import java.util.HashMap;

/**
 * Window overlays: a see-through GPU page in an application panel sub-window over the presenting
 * activity's window, which Rust knows as window 1. Other activities (camera, custom tabs) stay
 * above. Rust calls these from any thread; the registry is confined to the main thread.
 */
public final class GpuOverlays {
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final long WINDOW = 1;
    private static final String WORKER = "lingxia:overlay";
    private static final HashMap<Long, Overlay> OVERLAYS = new HashMap<>();
    /** Main thread: the page's host exists; Rust loads its module, connects it and sends what it held. */
    private static native void opened(long token);
    /** Window {@code window} can show no overlay: what it holds settles. */
    private static native void closed(long window);
    private static native void run(long task);
    /** The app went to the background or memory runs low: kept overlays close. */
    private static native void release();
    private static boolean watchingMemory;

    private GpuOverlays() {}

    /** Show overlay page {@code token} over {@code window}; Rust loads its module. */
    public static void open(long token, long window) {
        MAIN.post(() -> {
            Activity activity = LxApp.getCurrentActivity();
            if (window != WINDOW || activity == null || activity.isFinishing()) { closed(window); return; }
            Overlay overlay;
            try { overlay = new Overlay(token); }
            catch (Throwable error) {
                android.util.Log.w("LingXia.UI", "window overlay: " + error);
                closed(window);
                return;
            }
            OVERLAYS.put(token, overlay);
            overlay.attach(activity);
            opened(token);
            watchMemory(activity);
        });
    }

    /** Hide overlay {@code token}, keeping its window and surface for {@link #show}. */
    public static void hide(long token) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.get(token); if (overlay != null) overlay.hide(true); });
    }

    public static void show(long token) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.get(token); if (overlay != null) overlay.hide(false); });
    }

    /** UI_HIDDEN is the whole app leaving the screen; an activity's onStop is also a page push. */
    private static void watchMemory(Activity activity) {
        if (watchingMemory) return;
        watchingMemory = true;
        activity.getApplicationContext().registerComponentCallbacks(new android.content.ComponentCallbacks2() {
            @Override public void onTrimMemory(int level) { release(); }
            @Override public void onLowMemory() { release(); }
            @Override public void onConfigurationChanged(android.content.res.Configuration configuration) { }
        });
    }

    /** {@code block} takes every touch over the window; otherwise they reach the window below. */
    public static void input(long token, boolean block) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.get(token); if (overlay != null) overlay.block(block); });
    }

    /** Take key focus (the soft keyboard goes), or give it back to the window below. */
    public static void focus(long token, boolean focused) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.get(token); if (overlay != null) overlay.focus(focused); });
    }

    /** A runtime environment push for the overlay's page, as a GPU page takes one. */
    public static void updateEnvironment(long token, String update) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.get(token); if (overlay != null) overlay.page.updateEnvironment(update); });
    }

    public static void close(long token) {
        MAIN.post(() -> { Overlay overlay = OVERLAYS.remove(token); if (overlay != null) overlay.close(); });
    }

    /** Run boxed task {@code task} on the main thread after {@code delayMs}, exactly once. */
    public static void after(long task, long delayMs) {
        MAIN.postDelayed(() -> run(task), Math.max(0, delayMs));
    }

    /** Automation as a GPU page answers it, after the frames already posted. */
    public static void automate(long token, String request, long replyId) {
        MAIN.post(() -> {
            Overlay overlay = OVERLAYS.get(token);
            GpuPage.automate(overlay == null ? null : overlay.page, request, replyId);
        });
    }

    /** Main thread: the presenting activity changed; overlays move over it. */
    public static void restack() {
        Activity activity = LxApp.getCurrentActivity();
        if (activity == null) return;
        for (Overlay overlay : OVERLAYS.values()) overlay.attach(activity);
    }

    /** Main thread: {@code activity} is going; its overlays leave its window until the next one. */
    public static void activityDestroyed(Activity activity) {
        for (Overlay overlay : new ArrayList<>(OVERLAYS.values()))
            if (overlay.host == activity) overlay.detach();
    }

    private static final class Overlay {
        final GpuView page;
        final Root root;
        Activity host;
        /** Added to the host's window manager; attachment to the window follows a traversal later. */
        boolean added;
        boolean blocking, focused;
        /** Kept for the next item: transparent and paused, its surface retained. */
        boolean hidden;
        /** The view that had focus before a dialog took it. */
        View previous;
        /** The activity content a dialog hides from accessibility, as any modal does, and its prior setting. */
        View silenced;
        int silencedImportance;
        Object backCallback;

        Overlay(long token) {
            Context context = Lingxia.applicationContext();
            if (context == null) throw new IllegalStateException("SDK context unavailable");
            page = new GpuView(context, null, WORKER, token, new GpuView.Listener() {
                public void onLoaded() { }
                public void onFrame() { }
                public void onError(Throwable error) {
                    com.lingxia.app.LxLog.INSTANCE.e("LingXia.UI", "window overlay failed: " + error.getMessage(), null, "", "");
                    closed(WINDOW);
                }
            }, true);
            page.setSeeThrough();
            root = new Root(context);
            root.addView(page, new FrameLayout.LayoutParams(-1, -1));
        }

        void attach(Activity activity) {
            if (host == activity && added) return;
            IBinder token = activity.getWindow().getDecorView().getWindowToken();
            if (token == null) {
                // Its window is not up yet: attach once it is.
                activity.getWindow().getDecorView().post(() -> { if (OVERLAYS.containsValue(this)) attach(activity); });
                return;
            }
            detach();
            host = activity;
            activity.getWindowManager().addView(root, params(token));
            added = true;
            if (blocking) silence(activity);
            if (!hidden) page.resume();
        }

        void hide(boolean hide) {
            if (hidden == hide) return;
            hidden = hide;
            if (hide) { focus(false); block(false); page.pause(); }
            relayout();
            if (!hide && host != null) page.resume();
        }

        void detach() {
            if (host == null) return;
            if (focused) focus(false);
            silence(null);
            page.pause();
            if (added) host.getWindowManager().removeViewImmediate(root);
            added = false;
            host = null;
        }

        WindowManager.LayoutParams params(IBinder token) {
            WindowManager.LayoutParams params = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.MATCH_PARENT,
                WindowManager.LayoutParams.TYPE_APPLICATION_PANEL, flags(), PixelFormat.TRANSLUCENT);
            params.token = token;
            params.setTitle("LingXia overlay");
            // The window's layer, and the page's surface under it, stay but compose to nothing.
            params.alpha = hidden ? 0f : 1f;
            params.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING;
            if (Build.VERSION.SDK_INT >= 28)
                params.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            // The page pads itself by the insets, as a page under the system bars does.
            if (Build.VERSION.SDK_INT >= 30) params.setFitInsetsTypes(0);
            return params;
        }

        int flags() {
            int flags = WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS;
            if (!blocking) flags |= WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
            if (!focused) flags |= WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE;
            return flags;
        }

        void relayout() {
            if (host == null || !added) return;
            WindowManager.LayoutParams params = (WindowManager.LayoutParams) root.getLayoutParams();
            params.flags = flags();
            params.alpha = hidden ? 0f : 1f;
            host.getWindowManager().updateViewLayout(root, params);
        }

        void block(boolean block) {
            if (blocking == block) return;
            blocking = block;
            silence(block ? host : null);
            relayout();
        }

        void silence(Activity activity) {
            View content = activity == null ? null : activity.getWindow().getDecorView();
            if (content == silenced) return;
            if (silenced != null) silenced.setImportantForAccessibility(silencedImportance);
            silenced = content;
            if (content == null) return;
            silencedImportance = content.getImportantForAccessibility();
            content.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        }

        void focus(boolean focus) {
            if (focused == focus) return;
            focused = focus;
            if (focus && host != null) {
                previous = host.getCurrentFocus();
                if (previous != null) {
                    InputMethodManager input = host.getSystemService(InputMethodManager.class);
                    if (input != null) input.hideSoftInputFromWindow(previous.getWindowToken(), 0);
                }
            }
            relayout();
            if (focus) {
                page.requestFocus();
                registerBack();
            } else {
                unregisterBack();
                if (previous != null && previous.isAttachedToWindow()) previous.requestFocus();
                previous = null;
            }
        }

        /** Back goes to the page as Escape while a dialog holds focus. */
        void registerBack() {
            if (Build.VERSION.SDK_INT < 33 || backCallback != null) return;
            android.window.OnBackInvokedDispatcher dispatcher = root.findOnBackInvokedDispatcher();
            if (dispatcher == null) return;
            android.window.OnBackInvokedCallback callback = page::escape;
            dispatcher.registerOnBackInvokedCallback(android.window.OnBackInvokedDispatcher.PRIORITY_OVERLAY, callback);
            backCallback = callback;
        }

        void unregisterBack() {
            if (Build.VERSION.SDK_INT < 33 || backCallback == null) return;
            android.window.OnBackInvokedDispatcher dispatcher = root.findOnBackInvokedDispatcher();
            if (dispatcher != null) dispatcher.unregisterOnBackInvokedCallback((android.window.OnBackInvokedCallback) backCallback);
            backCallback = null;
        }

        void close() {
            detach();
            page.dispose();
        }

        /** The panel's root: Back reaches the page as Escape. */
        private final class Root extends FrameLayout {
            Root(Context context) { super(context); }
            @Override public boolean dispatchKeyEvent(KeyEvent event) {
                if (event.getKeyCode() == KeyEvent.KEYCODE_BACK && focused) {
                    if (event.getAction() == KeyEvent.ACTION_UP && !event.isCanceled()) page.escape();
                    return true;
                }
                return super.dispatchKeyEvent(event);
            }
            @Override protected void onDetachedFromWindow() {
                unregisterBack();
                super.onDetachedFromWindow();
            }
        }
    }
}
