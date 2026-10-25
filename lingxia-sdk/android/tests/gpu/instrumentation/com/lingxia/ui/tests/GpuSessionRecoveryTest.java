package com.lingxia.ui.tests;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import androidx.test.runner.lifecycle.ActivityLifecycleMonitorRegistry;
import androidx.test.runner.lifecycle.Stage;
import com.lingxia.ui.GpuView;
import com.lingxia.lxapp.GpuPage;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.Test;
import org.junit.runner.RunWith;
import static org.junit.Assert.*;

/** Fault injection stays in the test APK, outside the host and lxapp bundles. */
@RunWith(AndroidJUnit4.class)
public class GpuSessionRecoveryTest {
    private final Instrumentation instrumentation = InstrumentationRegistry.getInstrumentation();
    interface Check { boolean get() throws Exception; }
    interface Work<T> { T get() throws Exception; }

    private <T> T main(Work<T> work) throws Exception {
        AtomicReference<T> result = new AtomicReference<>();
        AtomicReference<Exception> failure = new AtomicReference<>();
        instrumentation.runOnMainSync(() -> {
            try { result.set(work.get()); } catch (Exception error) { failure.set(error); }
        });
        if (failure.get() != null) throw failure.get();
        return result.get();
    }
    private void await(Check check) throws Exception {
        long end = android.os.SystemClock.uptimeMillis() + 15000;
        do { if (check.get()) return; Thread.sleep(100); }
        while (android.os.SystemClock.uptimeMillis() < end);
        fail("GPU recovery deadline exceeded");
    }
    private static long host(Object view) throws Exception {
        Field field = GpuView.class.getDeclaredField("host"); field.setAccessible(true);
        return field.getLong(view);
    }
    private static boolean bridgeReady(Object view) throws Exception {
        Method method = GpuView.class.getDeclaredMethod("nativeBridgeReady", long.class); method.setAccessible(true);
        return (boolean)method.invoke(null, host(view));
    }
    private static Object field(Object target, String name) throws Exception {
        Field field = target.getClass().getDeclaredField(name); field.setAccessible(true);
        return field.get(target);
    }
    private static GpuView gpu(View view) {
        if (view instanceof GpuView && view.isShown()) return (GpuView)view;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) {
            GpuView found = gpu(((ViewGroup)view).getChildAt(i));
            if (found != null) return found;
        }
        return null;
    }
    private static Button reload(View view) {
        if (view instanceof Button && ((Button)view).getText().toString().equals(
                view.getContext().getString(com.lingxia.lxapp.R.string.lx_gpu_reload))) return (Button)view;
        if (view instanceof ViewGroup) for (int i = 0; i < ((ViewGroup)view).getChildCount(); i++) {
            Button found = reload(((ViewGroup)view).getChildAt(i));
            if (found != null) return found;
        }
        return null;
    }

    @Test public void hostReloadReconnectsLogicAndRejectsStaleRecovery() throws Exception {
        Intent launch = instrumentation.getTargetContext().getPackageManager().getLaunchIntentForPackage(
                instrumentation.getTargetContext().getPackageName());
        assertNotNull(launch);
        launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        instrumentation.getTargetContext().startActivity(launch);
        AtomicReference<GpuView> content = new AtomicReference<>();
        await(() -> main(() -> {
            for (Activity activity : ActivityLifecycleMonitorRegistry.getInstance().getActivitiesInStage(Stage.RESUMED)) {
                GpuView view = gpu(activity.getWindow().getDecorView());
                if (view != null && bridgeReady(view) && field(view, "snapshot") != null) {
                    content.set(view); return true;
                }
            }
            return false;
        }));
        GpuView view = content.get();
        long before = main(view::getConnectionGeneration);
        assertTrue(before > 0);
        long token = main(() -> {
            Field registry = GpuPage.class.getDeclaredField("PAGES"); registry.setAccessible(true);
            for (Map.Entry<?, ?> entry : ((Map<?, ?>)registry.get(null)).entrySet())
                if (field(entry.getValue(), "content") == view) return (Long)entry.getKey();
            throw new IllegalStateException("Current GPU page is not registered");
        });
        main(() -> {
            Method error = GpuView.class.getDeclaredMethod("nativeFail", long.class, String.class); error.setAccessible(true);
            error.invoke(null, host(view), "UI_TIMEOUT: instrumentation session fault");
            return null;
        });
        assertNotNull(main(() -> reload(view)));
        main(() -> { reload(view).performClick(); return null; });
        await(() -> main(() -> view.getConnectionGeneration() > before
                && bridgeReady(view) && field(view, "snapshot") != null));
        assertNull(main(() -> reload(view)));
        main(() -> {
            Method restart = GpuPage.class.getDeclaredMethod("restart", long.class, long.class); restart.setAccessible(true);
            assertEquals(false, restart.invoke(null, token, before));
            return null;
        });
    }
}
