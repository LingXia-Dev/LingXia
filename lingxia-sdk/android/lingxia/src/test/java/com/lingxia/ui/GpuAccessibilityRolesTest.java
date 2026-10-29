package com.lingxia.ui;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import com.lingxia.ui.generated.GpuSemantics;
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

    @Config(sdk = {28, 31})
    @Test public void selectSpeaksItsValueAndEditorsOfferPaste() throws Exception {
        GpuNativeLoadTest.Fake fake = new GpuNativeLoadTest.Fake();
        long host = GpuView.nativeHostCreate(fake, "test:jvm-select", false, 0);
        try {
            GpuView.nativeLoad(host, GpuNative.module(
                "function render(){const p=useLocal('p','');return h('column',{},"
                + "h('select',{testId:'p',bind:p,accessibilityLabel:'Billing plan',placeholder:'Choose a plan',"
                + "options:[{value:'m',label:'Monthly'},{value:'y',label:'Yearly'}],style:{height:40}}),"
                + "h('input',{testId:'e',accessibilityLabel:'Name',style:{height:40}}))}"));
            GpuView.nativeSetVisible(host, true);
            fake.pump(host, () -> fake.presented >= 1);
            Map<String, JSONObject> nodes = nodes(host);
            AccessibilityNodeInfo select = info(nodes.get("p"));
            if (android.os.Build.VERSION.SDK_INT >= 30) {
                assertEquals("Billing plan", String.valueOf(select.getContentDescription()));
                assertEquals("Choose a plan", String.valueOf(select.getStateDescription()));
            } else {
                assertEquals("Billing plan, Choose a plan", String.valueOf(select.getContentDescription()));
            }
            AccessibilityNodeInfo editor = info(nodes.get("e"));
            assertTrue(editor.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_PASTE));
            assertEquals(GpuAccessibility.CUT, GpuAccessibility.action(AccessibilityNodeInfo.ACTION_CUT));
            assertEquals(GpuAccessibility.PASTE, GpuAccessibility.action(AccessibilityNodeInfo.ACTION_PASTE));
        } finally {
            GpuView.nativeHostRelease(host);
            fake.release();
        }
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
                + "h('text',{testId:'s',role:'status'},'Saved '+n.value),"
                + "h('text',{},'Read the ',h('link',{testId:'l',onPress(){}},'terms')))}"));
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
            assertEquals("Link", String.valueOf(info(nodes.get("l")).getExtras().getCharSequence(GpuAccessibility.ROLE_DESCRIPTION)));
            assertTrue(GpuView.nativePointer(host, 0, 40, 40, 8, 0, 0, 50));
            GpuView.nativePointer(host, 2, 40, 40, 8, 0, 0, 50);
            fake.pump(host, () -> fake.events.contains("live:polite:Saved 1"));
        } finally {
            GpuView.nativeHostRelease(host);
            fake.release();
        }
    }

    /** Authored pressed, selected, expanded, busy and scroll states reach the node info. */
    @Test public void authoredStatesMapToNodeInfo() throws Exception {
        JSONObject toggle = new JSONObject().put("role", "button").put("name", "Bold").put("text", "B").put("clickable", true)
            .put("enabled", true).put(GpuSemantics.KEY_PRESSED, true).put(GpuSemantics.KEY_SELECTED, true);
        AccessibilityNodeInfo info = info(toggle);
        assertTrue(info.isCheckable()); assertTrue(info.isChecked()); assertTrue(info.isSelected());
        JSONObject disclosure = new JSONObject().put("role", "button").put("name", "More").put("clickable", true)
            .put("enabled", true).put(GpuSemantics.KEY_EXPANDED, false).put(GpuSemantics.KEY_BUSY, true);
        AccessibilityNodeInfo more = info(disclosure);
        assertTrue(more.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_EXPAND));
        assertEquals("More, Busy", more.getContentDescription().toString());
        assertEquals(GpuAccessibility.CLICK, GpuAccessibility.action(AccessibilityNodeInfo.ACTION_EXPAND));
        JSONObject region = new JSONObject().put("role", "scroll-view").put(GpuSemantics.KEY_SCROLL,
            new JSONObject().put("offset", 0).put("max_offset", 300).put("horizontal", false));
        assertTrue(info(region).isScrollable());
    }

    /** Every role the core projects has an Android class; a new one without a mapping fails here. */
    @Test public void everyProjectedRoleHasAnAndroidClass() throws Exception {
        for (java.lang.reflect.Field field : GpuSemantics.class.getFields()) {
            if (!field.getName().startsWith("ROLE_")) continue;
            String role = (String) field.get(null);
            assertNotNull(role, GpuAccessibility.className(role));
        }
        assertEquals("android.widget.CheckedTextView", GpuAccessibility.className(GpuSemantics.ROLE_OPTION));
        assertEquals("android.widget.GridView", GpuAccessibility.className(GpuSemantics.ROLE_GRID));
    }
}
