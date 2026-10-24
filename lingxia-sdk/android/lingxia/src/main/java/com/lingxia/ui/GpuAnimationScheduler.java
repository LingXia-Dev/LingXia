package com.lingxia.ui;

/** Schedules the shared timeline's next deadline; never owns animation time. */
final class GpuAnimationScheduler {
    interface Host {
        long now();
        void post(Runnable callback, long delayMillis);
        void remove(Runnable callback);
    }
    private final Host host;
    private final Runnable frame;
    private Runnable pending;

    GpuAnimationScheduler(Host host, Runnable frame) {
        this.host = host;
        this.frame = frame;
    }

    void cancel() {
        if (pending != null) host.remove(pending);
        pending = null;
    }

    void schedule(long delayMillis, long presentedAt) {
        cancel();
        if (delayMillis < 0) return;
        pending = new Runnable() {
            @Override public void run() {
                if (pending != this) return;
                pending = null;
                frame.run();
            }
        };
        long spent = Math.max(0, host.now() - presentedAt);
        host.post(pending, Math.max(0, delayMillis - spent));
    }
}
