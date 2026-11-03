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
    private static final int UPDATE_REQUIRED = 0, SESSION_FAILED = 1, UPDATE_FAILED = 2, DISPLAY_FAILED = 3;
    private static final int NONE = -1, RETRY = 0, RELOAD = 1;

    @Test public void retriesOnceAndRevokesDetachedActions() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger calls = new AtomicInteger();
        recovery.show(true, UPDATE_FAILED, RETRY, calls::incrementAndGet);
        LinearLayout panel = (LinearLayout)host.getChildAt(0);
        Button retry = (Button)panel.getChildAt(1);
        retry.performClick(); retry.performClick();
        assertEquals(1, calls.get());
        recovery.show(true, UPDATE_FAILED, RETRY, calls::incrementAndGet);
        retry = (Button)((LinearLayout)host.getChildAt(0)).getChildAt(1);
        recovery.clear();
        retry.performClick();
        assertEquals(1, calls.get());
        assertEquals(0, host.getChildCount());
    }

    @Test public void incompatibleHostsDoNotOfferAnImpossibleRetry() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        recovery.show(false, UPDATE_REQUIRED, NONE, () -> fail("Incompatible modules cannot be retried"));
        LinearLayout panel = (LinearLayout)host.getChildAt(0);
        assertEquals(1, panel.getChildCount());
        assertTrue(((android.widget.TextView)panel.getChildAt(0)).getText().toString().contains("Update the app"));
    }

    @Test public void interruptedSessionsOfferAnExplicitReload() {
        FrameLayout host = new FrameLayout(RuntimeEnvironment.getApplication());
        GpuRecovery recovery = new GpuRecovery(host);
        AtomicInteger restarts = new AtomicInteger();
        recovery.show(true, SESSION_FAILED, RELOAD, restarts::incrementAndGet);
        Button action = (Button)((LinearLayout)host.getChildAt(0)).getChildAt(1);
        assertEquals("Reload page", action.getText().toString());
        action.performClick();
        assertEquals(1, restarts.get());
    }

    @Test public void aFailedLoadShowsTheHostsRecoveryAndAReloadClearsIt() throws Exception {
        GpuNative.load();
        AtomicInteger errors = new AtomicInteger(), loads = new AtomicInteger();
        GpuView view = new GpuView(RuntimeEnvironment.getApplication(), "throw Error('broken module')", "test:recovery", new GpuView.Listener() {
            public void onLoaded() { loads.incrementAndGet(); }
            public void onFrame(String metadata) { }
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
