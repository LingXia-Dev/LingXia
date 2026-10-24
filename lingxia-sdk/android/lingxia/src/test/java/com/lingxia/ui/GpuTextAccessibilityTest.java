package com.lingxia.ui;

import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.FrameLayout;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.RuntimeEnvironment;
import org.robolectric.annotation.Config;
import java.util.ArrayList;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk={23,31})
public class GpuTextAccessibilityTest {
    private JSONObject node() throws Exception {
        return new JSONObject("{identity:'editor',tag:'textarea',input:{enabled:true,secure:true,placeholder:'Secret',edit_token:7,edit:{text:'a😀b',selection:[1,3]}}}");
    }
    private JSONObject frame(JSONObject node) throws Exception {
        return new JSONObject().put("nodes",new org.json.JSONArray().put(node));
    }
    @Test public void passwordMetadataUsesUtf16MaskAndSelectionWithoutHintAsValue() throws Exception {
        JSONObject node=node(); AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain();
        node.put("accessibility_label", "Account password");
        GpuTextAccessibility.populate(info,node);
        assertEquals("Account password", info.getContentDescription().toString());
        assertEquals("••••",info.getText().toString());
        assertEquals(1,info.getTextSelectionStart()); assertEquals(3,info.getTextSelectionEnd());
        assertTrue(info.isPassword()); assertTrue(info.isMultiLine());
        assertTrue((info.getActions() & AccessibilityNodeInfo.ACTION_SET_TEXT)!=0);
        node.getJSONObject("input").remove("edit"); node.getJSONObject("input").put("edit_token",0).put("value","").put("enabled",false);
        info=AccessibilityNodeInfo.obtain(); GpuTextAccessibility.populate(info,node);
        assertEquals("",info.getText().toString()); assertEquals(-1,info.getTextSelectionStart());
        assertEquals(0,info.getActions());
    }
    @Test public void presentedChangesEmitMaskedTextAndSelectionOnce() throws Exception {
        ArrayList<AccessibilityEvent> events=new ArrayList<>();
        FrameLayout parent=new FrameLayout(RuntimeEnvironment.getApplication()) {
            @Override public boolean requestSendAccessibilityEvent(View child,AccessibilityEvent event) {
                events.add(AccessibilityEvent.obtain(event)); return true;
            }
        };
        View view=new View(parent.getContext()); parent.addView(view);
        JSONObject before=frame(node()), next=frame(node());
        next.getJSONArray("nodes").getJSONObject(0).getJSONObject("input").getJSONObject("edit")
            .put("text","secret").put("selection",new org.json.JSONArray(new int[]{6,6}));
        android.view.accessibility.AccessibilityManager manager=(android.view.accessibility.AccessibilityManager)view.getContext().getSystemService(android.content.Context.ACCESSIBILITY_SERVICE);
        org.robolectric.Shadows.shadowOf(manager).setEnabled(false);
        GpuTextAccessibility.events(view,before,next,i -> 42); assertTrue(events.isEmpty());
        org.robolectric.Shadows.shadowOf(manager).setEnabled(true);
        GpuTextAccessibility.events(view,before,next,i -> 42);
        assertEquals(2,events.size());
        assertEquals(AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED,events.get(0).getEventType());
        assertEquals("••••",events.get(0).getBeforeText().toString());
        assertEquals("••••••",events.get(0).getText().get(0).toString());
        assertEquals(6,events.get(1).getToIndex()); assertTrue(events.get(1).isPassword());
        GpuTextAccessibility.events(view,next,next,i -> 42); assertEquals(2,events.size());
    }
}
