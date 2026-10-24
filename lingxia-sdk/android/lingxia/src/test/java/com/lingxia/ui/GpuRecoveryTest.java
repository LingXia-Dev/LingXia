package com.lingxia.ui;

import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk=28)
@org.robolectric.annotation.LooperMode(org.robolectric.annotation.LooperMode.Mode.PAUSED)
public class GpuRecoveryTest {
    private static class DeferredWorker extends java.util.concurrent.AbstractExecutorService {
        public void execute(Runnable task) { }
        public void shutdown() { }
        public java.util.List<Runnable> shutdownNow() { return java.util.Collections.emptyList(); }
        public boolean isShutdown() { return false; }
        public boolean isTerminated() { return false; }
        public boolean awaitTermination(long timeout, java.util.concurrent.TimeUnit unit) { return false; }
    }

    @Test public void lateFailureCannotStopAReplacementSession() throws Exception {
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null);
        java.lang.reflect.Field generation = GpuView.class.getDeclaredField("inputPageGeneration"); generation.setAccessible(true);
        java.lang.reflect.Method post = GpuView.class.getDeclaredMethod("postError", Throwable.class, long.class); post.setAccessible(true);
        post.invoke(view, new IllegalStateException("UI_TIMEOUT: old click"), generation.getLong(view));
        view.reload();
        org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle();
        assertEquals(2, view.getChildCount());
        java.lang.reflect.Field failed = GpuView.class.getDeclaredField("sessionFailed"); failed.setAccessible(true);
        assertFalse(failed.getBoolean(view));
        post.invoke(view, new IllegalStateException("UI_LAYOUT: current failure"), generation.getLong(view));
        org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper()).idle();
        assertEquals(3, view.getChildCount());
        view.dispose();
    }

    @Test public void repeatedTimeoutsKeepHostRecoveryAvailableBeforeBridgeReadiness() throws Exception {
        java.util.concurrent.atomic.AtomicLong restarted = new java.util.concurrent.atomic.AtomicLong();
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", new DeferredWorker(), new GpuView.Listener() {
            public void onLoaded() { }
            public void onFrame(String metadata) { }
            public void onError(Throwable error) { }
            public boolean onRestart(long generation) { restarted.set(generation); return true; }
        }, true);
        java.lang.reflect.Field ready = GpuView.class.getDeclaredField("ready"); ready.setAccessible(true); ready.set(view, true);
        java.lang.reflect.Field generation = GpuView.class.getDeclaredField("bridgeGeneration"); generation.setAccessible(true); generation.set(view, 9L);
        java.lang.reflect.Method failure = GpuView.class.getDeclaredMethod("showError", Throwable.class); failure.setAccessible(true);
        failure.invoke(view, new IllegalStateException("UI_TIMEOUT: interrupted"));
        failure.invoke(view, new IllegalStateException("UI_TIMEOUT: queued callback"));
        assertFalse(ready.getBoolean(view));
        ((Button)((LinearLayout)view.getChildAt(2)).getChildAt(1)).performClick();
        assertEquals(9L, restarted.get());
        assertEquals(9L, generation.getLong(view));
        view.dispose();
    }
    @Test public void retriesOnceAndRevokesDetachedActions() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger calls = new AtomicInteger();
        recovery.show(true, GpuRecovery.Kind.CANDIDATE, calls::incrementAndGet);
        LinearLayout panel = (LinearLayout)host.getChildAt(0);
        Button retry = (Button)panel.getChildAt(1);
        retry.performClick(); retry.performClick();
        assertEquals(1, calls.get());
        recovery.show(true, GpuRecovery.Kind.CANDIDATE, calls::incrementAndGet);
        retry = (Button)((LinearLayout)host.getChildAt(0)).getChildAt(1);
        recovery.clear();
        retry.performClick();
        assertEquals(1, calls.get());
        assertEquals(0, host.getChildCount());
    }

    @Test public void incompatibleHostsDoNotOfferAnImpossibleRetry() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        recovery.show(false, GpuRecovery.Kind.COMPATIBILITY, () -> fail("Incompatible modules cannot be retried"));
        LinearLayout panel = (LinearLayout)host.getChildAt(0);
        assertEquals(1, panel.getChildCount());
        assertTrue(((android.widget.TextView)panel.getChildAt(0)).getText().toString().contains("Update the app"));
    }

    @Test public void interruptedSessionsOfferAnExplicitReload() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        GpuRecovery.Kind kind = GpuRecovery.kind(new IllegalStateException("UI_TIMEOUT: discard the interrupted page"));
        assertEquals(GpuRecovery.Kind.SESSION, kind);
        AtomicInteger restarts = new AtomicInteger();
        recovery.show(true, kind, restarts::incrementAndGet);
        Button action = (Button)((LinearLayout)host.getChildAt(0)).getChildAt(1);
        assertEquals("Reload page", action.getText().toString());
        action.performClick();
        assertEquals(1, restarts.get());
    }

    @Test public void candidateRetryPreservesSurfaceSessionAndCommittedSnapshot() throws Exception {
        java.util.concurrent.ExecutorService worker = new DeferredWorker();
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "", worker, null, true);
        android.view.View surface = view.getChildAt(0);
        int spec = android.view.View.MeasureSpec.makeMeasureSpec(400, android.view.View.MeasureSpec.EXACTLY);
        view.measure(spec, spec); view.layout(0, 0, 400, 400);
        java.lang.reflect.Field ready = GpuView.class.getDeclaredField("ready"); ready.setAccessible(true); ready.set(view, true);
        java.lang.reflect.Field bridge = GpuView.class.getDeclaredField("bridgeReady"); bridge.setAccessible(true); bridge.set(view, true);
        java.lang.reflect.Field snapshot = GpuView.class.getDeclaredField("snapshot"); snapshot.setAccessible(true);
        org.json.JSONObject committed = new org.json.JSONObject("{\"nodes\":[]}"); snapshot.set(view, committed);
        java.lang.reflect.Field generation = GpuView.class.getDeclaredField("inputPageGeneration"); generation.setAccessible(true);
        long previousGeneration = generation.getLong(view);
        java.lang.reflect.Method failure = GpuView.class.getDeclaredMethod("showError", Throwable.class); failure.setAccessible(true);
        failure.invoke(view, new IllegalStateException("UI_LAYOUT: invalid size"));
        assertEquals(3, view.getChildCount());
        java.lang.reflect.Field pending = GpuView.class.getDeclaredField("pending"); pending.setAccessible(true);
        view.measure(spec, spec); view.layout(0, 0, 400, 400);
        assertFalse("Feedback layout must not schedule an automatic retry", pending.getBoolean(view));
        ((Button)((LinearLayout)view.getChildAt(2)).getChildAt(1)).performClick();
        assertTrue(pending.getBoolean(view));
        assertSame(surface, view.getChildAt(0));
        assertSame(committed, snapshot.get(view));
        assertEquals(previousGeneration, generation.getLong(view));
        assertTrue(bridge.getBoolean(view));
        view.dispose();
        assertEquals(2, view.getChildCount());
    }
}
