package com.lingxia.ui;

import android.os.Handler;

/** Main-thread deadline; connection replacement revokes the old timer. */
final class GpuBootstrap {
    private final Handler handler;
    private Runnable pending;

    GpuBootstrap(Handler handler) { this.handler = handler; }

    void begin(Runnable timeout) {
        cancel();
        pending = () -> {
            pending = null;
            timeout.run();
        };
        handler.postDelayed(pending, 5000);
    }

    void cancel() {
        if (pending != null) handler.removeCallbacks(pending);
        pending = null;
    }
}
