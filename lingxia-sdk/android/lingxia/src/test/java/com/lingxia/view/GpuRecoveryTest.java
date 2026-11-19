package com.lingxia.view;

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
    private static final int UPDATE_REQUIRED = 0, SESSION_FAILED = 1, UPDATE_FAILED = 2, DISPLAY_FAILED = 3, SLOW_LOAD = 4;
    private static final int NONE = -1, RETRY = 0, RELOAD = 1, KEEP_WAITING = 2;

    @Test public void aSlowLoadOffersKeepWaitingAndBackOnThePagesCanvas() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        recovery.setColors(0xFFF5F6FA, 0xFF141821, 0xFF008347);
        AtomicInteger waits = new AtomicInteger();
        recovery.show(false, SLOW_LOAD, KEEP_WAITING, waits::incrementAndGet, () -> true);
        LinearLayout panel = (LinearLayout)host.getChildAt(0);
        android.widget.TextView message = recovery.message();
        assertEquals("This page is taking longer than usual to load.", message.getText().toString());
        assertEquals(0xFF141821, message.getCurrentTextColor());
        assertEquals(0xFFF5F6FA, ((android.graphics.drawable.ColorDrawable)panel.getBackground()).getColor());
        Button wait = recovery.action();
        assertEquals("Keep waiting", wait.getText().toString());
        assertEquals("text buttons in the accent colour", 0xFF008347, wait.getCurrentTextColor());
        assertEquals("Go back", recovery.back().getText().toString());
        float density = RuntimeEnvironment.getApplication().getResources().getDisplayMetrics().density;
        assertTrue("a 48 dp touch target", wait.getMinimumHeight() >= Math.round(48 * density));
        wait.performClick();
        assertEquals(1, waits.get());
        assertEquals("the label stays while the panel closes", "Keep waiting", wait.getText().toString());
    }

    @Test public void retriesOnceAndRevokesDetachedActions() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger calls = new AtomicInteger();
        recovery.show(true, UPDATE_FAILED, RETRY, calls::incrementAndGet, null);
        Button retry = recovery.action();
        retry.performClick(); retry.performClick();
        assertEquals(1, calls.get());
        recovery.show(true, UPDATE_FAILED, RETRY, calls::incrementAndGet, null);
        retry = recovery.action();
        recovery.clear();
        retry.performClick();
        assertEquals(1, calls.get());
        assertEquals(0, host.getChildCount());
    }

    @Test public void incompatibleHostsDoNotOfferAnImpossibleRetry() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        recovery.show(false, UPDATE_REQUIRED, NONE, () -> fail("Incompatible modules cannot be retried"), null);
        assertNull(recovery.action());
        assertNull(recovery.back());
        assertTrue(recovery.message().getText().toString().contains("Update the app"));
    }

    @Test public void backShowsOnlyWhenOfferedAndRetriesAfterARefusal() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger backs = new AtomicInteger();
        recovery.show(false, DISPLAY_FAILED, RETRY, () -> { }, () -> backs.incrementAndGet() > 1);
        assertNotNull(recovery.action());
        Button back = recovery.back();
        assertEquals("Go back", back.getText().toString());
        back.performClick();
        assertTrue("a refused back can be pressed again", back.isEnabled());
        back.performClick();
        assertFalse(back.isEnabled());
        assertEquals(2, backs.get());
        recovery.show(false, UPDATE_REQUIRED, NONE, null, null);
        assertNull("nowhere to go back to", recovery.back());
    }

    @Test public void interruptedSessionsOfferAnExplicitReload() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger restarts = new AtomicInteger();
        recovery.show(true, SESSION_FAILED, RELOAD, restarts::incrementAndGet, null);
        Button action = recovery.action();
        assertEquals("Reload page", action.getText().toString());
        action.performClick();
        assertEquals(1, restarts.get());
    }

    @Test public void aFailedLoadShowsTheHostsRecoveryAndAReloadClearsIt() throws Exception {
        GpuNative.load();
        AtomicInteger errors = new AtomicInteger(), loads = new AtomicInteger();
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "throw Error('broken module')", "test:recovery", new GpuView.Listener() {
            public void onLoaded() { loads.incrementAndGet(); }
            public void onFrame() { }
            public void onError(Throwable error) { errors.incrementAndGet(); }
        });
        android.view.View surface = view.getChildAt(0), feedback = view.getChildAt(1);
        GpuNative.settleUntil(() -> errors.get() > 0);
        assertEquals(1, errors.get());
        assertEquals("the recovery panel sits above surface and feedback", 3, view.getChildCount());
        LinearLayout panel = (LinearLayout)view.getChildAt(2);
        assertEquals(RuntimeEnvironment.getApplication().getString(com.lingxia.lxapp.R.string.lx_gpu_display_failed),
            ((android.widget.TextView)panel.getChildAt(0)).getText().toString());
        assertEquals("nothing loaded, so no retry is offered", 1, panel.getChildCount());
        view.reload(GpuNative.module("function render(){return h('text',{},'ok')}"));
        assertEquals(2, view.getChildCount());
        assertSame(surface, view.getChildAt(0)); assertSame(feedback, view.getChildAt(1));
        GpuNative.settleUntil(() -> loads.get() > 0);
        assertEquals(1, loads.get());
        assertEquals(2, view.getChildCount());
        view.dispose();
    }
}
