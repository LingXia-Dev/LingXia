package com.lingxia.ui;

import android.os.Handler;
import android.os.Looper;
import java.time.Duration;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.LooperMode;
import static org.junit.Assert.*;
import static org.robolectric.Shadows.shadowOf;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk=28)
@LooperMode(LooperMode.Mode.PAUSED)
public class GpuBootstrapTest {
    private static class DeferredWorker extends java.util.concurrent.AbstractExecutorService {
        public void execute(Runnable task) {}
        public void shutdown() {}
        public java.util.List<Runnable> shutdownNow() { return java.util.Collections.emptyList(); }
        public boolean isShutdown() { return false; }
        public boolean isTerminated() { return false; }
        public boolean awaitTermination(long timeout, java.util.concurrent.TimeUnit unit) { return false; }
    }

    @Test public void viewShowsTimeoutAndRecoversOnlyForItsCurrentConnection() throws Exception {
        GpuView view = new GpuView(org.robolectric.RuntimeEnvironment.getApplication(), "", new DeferredWorker(), null, true);
        android.view.View surface = view.getChildAt(0);
        int importance = surface.getImportantForAccessibility();
        view.connect(1, "first");
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(5));
        assertEquals(3, view.getChildCount());
        assertTrue(((android.widget.TextView)view.getChildAt(2)).getText().toString().contains("taking longer"));
        assertEquals(android.view.View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS, surface.getImportantForAccessibility());
        view.connect(2, "replacement");
        assertEquals(2, view.getChildCount());
        java.lang.reflect.Method update = GpuView.class.getDeclaredMethod("publishBridgeUpdate", long.class, String.class);
        update.setAccessible(true);
        String ready = "{\"ready\":true,\"changed\":false,\"outbound\":[],\"errors\":[]}";
        update.invoke(view, 1L, ready);
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(5));
        assertEquals(3, view.getChildCount());
        update.invoke(view, 2L, ready);
        shadowOf(Looper.getMainLooper()).idle();
        assertEquals(2, view.getChildCount());
        assertEquals(importance, surface.getImportantForAccessibility());
        view.connect(3, "disposed");
        view.dispose();
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(10));
        assertEquals(2, view.getChildCount());
    }

    @Test public void missingReadinessReportsOnceAfterFiveSeconds() {
        AtomicInteger failures = new AtomicInteger();
        GpuBootstrap deadline = new GpuBootstrap(new Handler(Looper.getMainLooper()));
        deadline.begin(failures::incrementAndGet);
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(4999));
        assertEquals(0, failures.get());
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofMillis(1));
        assertEquals(1, failures.get());
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(60));
        assertEquals(1, failures.get());
    }

    @Test public void readyDisposalAndReplacementRevokePreviousDeadlines() {
        AtomicInteger old = new AtomicInteger(), fresh = new AtomicInteger();
        GpuBootstrap deadline = new GpuBootstrap(new Handler(Looper.getMainLooper()));
        deadline.begin(old::incrementAndGet);
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(4));
        deadline.begin(fresh::incrementAndGet);
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(2));
        assertEquals(0, old.get());
        assertEquals(0, fresh.get());
        deadline.cancel();
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(10));
        assertEquals(0, fresh.get());
        deadline.begin(fresh::incrementAndGet);
        shadowOf(Looper.getMainLooper()).idleFor(Duration.ofSeconds(5));
        assertEquals(1, fresh.get());
    }
}
