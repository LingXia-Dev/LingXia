package com.lingxia.ui;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

/** Static-text selection: handles from the frame, drag, select all and copy through the host. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuStaticSelectionTest {
    @Before public void load() { GpuNative.load(); }

    private static JSONObject selected(GpuNativeLoadTest.Fake fake) throws Exception {
        JSONArray nodes = new JSONObject(fake.frames.get(fake.frames.size() - 1)).getJSONArray("nodes");
        for (int i = 0; i < nodes.length(); i++)
            if (nodes.getJSONObject(i).optJSONObject("text_selection") != null) return nodes.getJSONObject(i);
        return null;
    }

    private static JSONObject node(GpuNativeLoadTest.Fake fake, String id) throws Exception {
        JSONArray nodes = new JSONObject(fake.frames.get(fake.frames.size() - 1)).getJSONArray("nodes");
        for (int i = 0; i < nodes.length(); i++)
            if (id.equals(nodes.getJSONObject(i).optString("test_id"))) return nodes.getJSONObject(i);
        return null;
    }

    @Test public void longPressSelectsAndHandlesDragSelectAllAndCopy() throws Exception {
        GpuNativeLoadTest.Fake fake = new GpuNativeLoadTest.Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm-selection", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){return h('text',{testId:'p',style:{userSelect:'text',fontSize:16,width:90}},'hello brave new world')}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            JSONArray b = node(fake, "p").getJSONArray("bounds");
            // Environment scale 2: pointer coordinates are device pixels.
            float x = (float)(b.getDouble(0) + 8) * 2, y = (float)(b.getDouble(1) + 8) * 2;
            GpuView.nativePointer(host, 0, x, y, 8, 0, 0, 50);
            GpuView.nativeLongPress(host);
            fake.pump(host, () -> { try { return selected(fake) != null; } catch (Exception e) { throw new AssertionError(e); } });
            JSONObject node = selected(fake);
            float[] focus = GpuView.nativeHandleEndpoint(node.toString(), 1);
            assertNotNull("focus handle", focus);
            float[] hit = GpuView.nativeHandleHit(node.toString(), focus[0], focus[1] + 8, false);
            assertNotNull("handle hit", hit);
            assertEquals(1, (int)hit[0]);
            GpuView.nativePointer(host, 3, x, y, 8, 0, 0, 50);
            int frames = fake.frames.size();
            GpuView.nativeSetTextSelection(host, node.getString("identity"), 0, 21);
            fake.pump(host, () -> fake.frames.size() > frames);
            fake.events.clear();
            GpuView.nativeCopyTextSelection(host, 7);
            fake.pump(host, () -> fake.events.stream().anyMatch(e -> e.startsWith("reply:")));
            String reply = fake.events.stream().filter(e -> e.startsWith("reply:")).findFirst().get().substring(6);
            assertEquals("hello brave new world", new JSONObject(reply).getString("text"));
            fake.pump(host, () -> { try { return selected(fake) == null; } catch (Exception e) { throw new AssertionError(e); } });
            assertNull("copy drops the selection", selected(fake));
        } finally {
            GpuView.nativeHostRelease(host);
            fake.release();
        }
    }
}
