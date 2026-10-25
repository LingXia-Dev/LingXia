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
        // Each Robolectric SDK sandbox has its own classloader, and a library binds to one; load a copy per sandbox.
        try {
            java.io.File copy = java.io.File.createTempFile("lingxia-ui-", path.substring(path.lastIndexOf('.')));
            copy.deleteOnExit();
            Files.copy(Paths.get(path), copy.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            System.load(copy.getAbsolutePath());
        } catch (java.io.IOException error) { throw new IllegalStateException(error); }
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

    /** Run main-looper work the View worker posts until {@code done} holds. */
    static void settleUntil(java.util.function.BooleanSupplier done) throws InterruptedException {
        org.robolectric.shadows.ShadowLooper looper = org.robolectric.Shadows.shadowOf(android.os.Looper.getMainLooper());
        long deadline = System.currentTimeMillis() + 20000;
        while (!done.getAsBoolean()) {
            if (System.currentTimeMillis() > deadline) throw new AssertionError("View worker deadline");
            Thread.sleep(20);
            looper.idle();
        }
        settle();
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
