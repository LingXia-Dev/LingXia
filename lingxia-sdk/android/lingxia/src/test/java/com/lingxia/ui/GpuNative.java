package com.lingxia.ui;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
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

    /** A page module over the bundled native-test runtime. */
    static String module(String render) throws Exception {
        // Gradle runs unit tests from the module directory; the shared target may live in another checkout.
        String runtime = new String(Files.readAllBytes(Paths.get(System.getProperty("user.dir"))
            .resolve("../../../crates/lingxia-ui/src/session/tests/runtime.js").normalize()), StandardCharsets.UTF_8);
        return runtime + "\n" + render + "\n__lingxiaPublish(render);";
    }

    /** Run main-looper work the View worker posts until it stays quiet. */
    static void settle() throws InterruptedException {
        org.robolectric.shadows.ShadowLooper looper = org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper());
        for (int quiet = 0; quiet < 10;) {
            Thread.sleep(20);
            if (looper.isIdle()) quiet++; else { quiet = 0; looper.idle(); }
        }
    }
}
