package com.lingxia.ui;

import org.junit.Assume;

/** Loads the host build of the shared core once per JVM; tests skip when it is absent. */
final class GpuNative {
    private static boolean loaded;

    static synchronized void load() {
        String path = System.getProperty("lingxia.ui.native", "");
        Assume.assumeTrue("set -PlingxiaUiNative to the host liblingxia_ui library", !path.isEmpty());
        if (loaded) return;
        System.load(path);
        GpuView.initializeNative();
        loaded = true;
    }
}
