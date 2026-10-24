package com.lingxia.ui;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

/** The JNI codec drives the real shared core: lifecycle tickets, frames, input and semantics. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuNativeLoadTest {
    @Before public void loadCore() { GpuNative.load(); }

    private static String module(String render) throws Exception {
        String runtime = new String(Files.readAllBytes(Paths.get(System.getProperty("lingxia.ui.native"))
            .getParent().getParent().getParent().resolve("crates/lingxia-ui/src/session/tests/runtime.js")), StandardCharsets.UTF_8);
        return runtime + "\n" + render + "\n__lingxiaPublish(render);";
    }

    @Test public void lifecycleTicketsComeFromTheSharedCore() throws Exception {
        long lifecycle = GpuView.nativeLifecycleCreate(false);
        long page = GpuView.nativeLifecycle(lifecycle, 0, 0);
        assertTrue(page > 0);
        assertEquals(1, GpuView.nativeLifecycle(lifecycle, 1, page));
        assertEquals(0, GpuView.nativeLifecycle(lifecycle, 11, page - 1));
        JSONObject recovery = new JSONObject(GpuView.nativeLifecycleFail(lifecycle, "UI_TIMEOUT: budget"));
        assertEquals("Session", recovery.getString("kind"));
        assertEquals("Reload", recovery.getString("retry"));
        GpuView.nativeLifecycleRelease(lifecycle);
    }

    private static Object call(String name, Class<?>[] types, Object... args) throws Exception {
        java.lang.reflect.Method method = GpuView.class.getDeclaredMethod(name, types); method.setAccessible(true);
        return method.invoke(null, args);
    }

    @Test public void presentedFramesDriveTapsAndSemanticsThroughTheCore() throws Exception {
        long page = (long)call("nativeLoad", new Class<?>[]{String.class}, module(
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},'Count '+n.value))}"));
        Class<?>[] render = {long.class, int.class, int.class, float.class, boolean.class, boolean.class, String.class, long.class, long.class};
        call("nativeRender", render, page, 200, 300, 2f, true, false, "{}", 1L, 0L);
        long frame = (long)call("nativeTakeFrame", new Class<?>[]{long.class}, page);
        assertNotEquals(0, frame);
        long interaction = GpuView.nativeInteractionCreate();
        JSONObject presented = new JSONObject(GpuView.nativePresented(interaction, frame, 0, "{}", 1));
        assertTrue(presented.getJSONArray("effects").toString().contains("content-changed"));
        JSONObject roots = new JSONObject(GpuView.nativeQuery(interaction, 4, 0));
        int button = roots.getJSONArray("roots").getInt(0);
        JSONObject node = new JSONObject(GpuView.nativeQuery(interaction, 5, button));
        assertEquals("button", node.getString("role"));
        assertEquals("Count 0", node.getString("name"));
        GpuView.nativePointer(interaction, 0, 40, 40, 8, 0, 0, 50, 1);
        JSONObject up = new JSONObject(GpuView.nativePointer(interaction, 2, 40, 40, 8, 0, 0, 50, 1));
        assertTrue(up.getJSONArray("effects").toString().contains("\"dispatch\""));
        JSONArray dispatches = new JSONArray(GpuView.nativeQuery(interaction, 1, 0));
        JSONObject click = dispatches.getJSONObject(0);
        assertEquals("click", click.getString("kind"));
        assertTrue((boolean)call("nativeClick", new Class<?>[]{long.class, long.class, long.class}, page, click.getLong("commit"), click.getLong("handle")));
        call("nativeRender", render, page, 200, 300, 2f, true, false, "{}", 1L, 0L);
        GpuView.nativePresented(interaction, (long)call("nativeTakeFrame", new Class<?>[]{long.class}, page), 0, "{}", 1);
        assertEquals("Count 1", new JSONObject(GpuView.nativeQuery(interaction, 5, button)).getString("name"));
        GpuView.nativeInteractionRelease(interaction);
        call("nativeRelease", new Class<?>[]{long.class}, page);
    }
}
