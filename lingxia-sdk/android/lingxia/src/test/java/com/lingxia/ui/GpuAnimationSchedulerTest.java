package com.lingxia.ui;

import org.junit.Test;
import static org.junit.Assert.*;

public class GpuAnimationSchedulerTest {
    private static final class Host implements GpuAnimationScheduler.Host {
        long time, delay;
        Runnable callback;
        int removed, frames;
        public long now() { return time; }
        public void post(Runnable callback, long delay) { this.callback = callback; this.delay = delay; }
        public void remove(Runnable callback) { removed++; if (this.callback == callback) this.callback = null; }
        GpuAnimationScheduler scheduler() { return new GpuAnimationScheduler(this, () -> frames++); }
    }

    @Test public void acknowledgementLatencyDoesNotPushTheDeadlineBack() {
        Host host = new Host(); GpuAnimationScheduler scheduler = host.scheduler();
        host.time = 125; scheduler.schedule(100, 100);
        assertEquals(75, host.delay);
        host.time = 250; scheduler.schedule(100, 100);
        assertEquals(0, host.delay);
        host.callback.run(); assertEquals(1, host.frames);
        host.callback.run(); assertEquals(1, host.frames);
    }

    @Test public void cancellationAndReplacementRejectAlreadyQueuedCallbacks() {
        Host host = new Host(); GpuAnimationScheduler scheduler = host.scheduler();
        scheduler.schedule(0, 0); Runnable old = host.callback;
        scheduler.schedule(450, 0); Runnable current = host.callback;
        assertEquals(1, host.removed);
        old.run(); assertEquals(0, host.frames);
        scheduler.cancel(); current.run(); assertEquals(0, host.frames);
        assertNull(host.callback);
        scheduler.schedule(0, 0); host.callback.run(); assertEquals(1, host.frames);
    }

    @Test public void completedEffectsLeaveNoWakeupAndDoNotCreateAPeriodicLoop() {
        Host host = new Host(); GpuAnimationScheduler scheduler = host.scheduler();
        scheduler.schedule(450, 0); Runnable old = host.callback;
        scheduler.schedule(-1, 20); assertNull(host.callback);
        old.run(); assertEquals(0, host.frames);
        scheduler.schedule(0, 20); host.callback.run();
        assertEquals(1, host.frames);
        scheduler.cancel(); assertEquals(1, host.removed);
    }
}
