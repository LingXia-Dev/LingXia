package com.lingxia.ui;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.HashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;

/** Authored roles reach TalkBack as headings, collections, panes and live regions. */
@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuAccessibilityRolesTest {
    @Before public void load() { GpuNative.load(); }

    private static Map<String, JSONObject> nodes(long host) throws Exception {
        Map<String, JSONObject> found = new HashMap<>();
        JSONArray pending = new JSONObject(GpuView.nativeSemanticRoots(host)).getJSONArray("roots");
        for (int i = 0; i < pending.length(); i++) {
            JSONObject node = new JSONObject(GpuView.nativeSemanticNode(host, pending.getInt(i)));
            found.put(node.optString("test_id"), node);
            JSONArray children = node.optJSONArray("children");
            for (int c = 0; children != null && c < children.length(); c++) pending.put(children.getInt(c));
        }
        return found;
    }

    private static AccessibilityNodeInfo info(JSONObject node) {
        View view = new View(RuntimeEnvironment.getApplication());
        AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain();
        GpuAccessibility.populate(info, view, node, null, 2, false, "test");
        return info;
    }

    @Test public void headingsListsDialogsAndLiveRegionsMapToNodeInfo() throws Exception {
        GpuNativeLoadTest.Fake fake = new GpuNativeLoadTest.Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm-roles", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){const n=useLocal('n',0);return h('column',{},"
                + "h('button',{testId:'add',onClick:()=>n.set(n.value+1),style:{height:60}},'Add'),"
                + "h('text',{testId:'h',role:'heading',headingLevel:3},'Plans'),"
                + "h('column',{testId:'list',role:'list'},h('row',{testId:'a',role:'listitem'},h('text',{},'Water')),h('row',{testId:'b',role:'listitem'},h('text',{},'Power'))),"
                + "h('column',{testId:'d',role:'dialog',accessibilityLabel:'Confirm'},h('text',{},'Sure?')),"
                + "h('text',{testId:'s',role:'status'},'Saved '+n.value))}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            Map<String, JSONObject> nodes = nodes(host);
            assertTrue(info(nodes.get("h")).isHeading());
            AccessibilityNodeInfo list = info(nodes.get("list"));
            assertEquals("android.widget.ListView", list.getClassName());
            assertEquals(2, list.getCollectionInfo().getRowCount());
            assertEquals(1, info(nodes.get("b")).getCollectionItemInfo().getRowIndex());
            AccessibilityNodeInfo dialog = info(nodes.get("d"));
            assertEquals("Confirm", dialog.getPaneTitle());
            assertTrue(!dialog.isClickable());
            assertEquals(View.ACCESSIBILITY_LIVE_REGION_POLITE, info(nodes.get("s")).getLiveRegion());
            assertTrue(GpuView.nativePointer(host, 0, 40, 40, 8, 0, 0, 50));
            GpuView.nativePointer(host, 2, 40, 40, 8, 0, 0, 50);
            fake.pump(host, () -> fake.events.contains("live:polite:Saved 1"));
        } finally {
            GpuView.nativeHostRelease(host);
            fake.release();
        }
    }
}
