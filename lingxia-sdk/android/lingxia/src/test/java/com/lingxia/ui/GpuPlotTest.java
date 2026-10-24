package com.lingxia.ui;

import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk=28)
public class GpuPlotTest {
    private JSONObject node(String kind) throws Exception {
        return new JSONObject("{identity:'chart',tag:'plot',bounds:[10,20,100,100],visible_bounds:[10,20,100,100],plot:{kind:'"+kind+"',values:[10,0,10],selected_index:-1,on_select:7}}");
    }

    @Test public void clearingRequiresAnActiveSelectionAndExposesAnAccessibleAction() throws Exception {
        JSONObject node=node("pie");
        assertFalse(GpuPlot.canClear(node));
        GpuPlot.plot(node).put("selected_index",0);
        assertTrue(GpuPlot.canClear(node));
        android.view.accessibility.AccessibilityNodeInfo info=android.view.accessibility.AccessibilityNodeInfo.obtain();
        GpuPlot.populate(info,node);
        assertTrue(info.getActionList().contains(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_SELECTION));
        info.recycle();
        GpuPlot.plot(node).remove("on_select");assertFalse(GpuPlot.canClear(node));
        assertFalse(GpuPlot.canClear(null));
    }

    @Test public void configuredDonutUsesSnapshotCenterAndRadii() throws Exception {
        JSONObject before=node("pie"),after=node("pie");
        GpuPlot.plot(after).put("pie_layout",new JSONObject("{center:[0.5,0.44],innerRadius:0.4,outerRadius:0.68}"));
        assertEquals(-1,GpuClick.find(before,new JSONArray().put(after)));
    }
    @Test public void queuedSelectionOnlyRebindsAnUnchangedPlot() throws Exception {
        JSONObject before=node("line"),after=node("line");
        GpuPlot.plot(after).put("on_select",19);
        assertEquals(0,GpuClick.find(before,new JSONArray().put(after)));
        assertEquals(7,GpuPlot.plot(before).getInt("on_select"));
        GpuPlot.plot(after).put("selected_index",1);
        assertEquals(-1,GpuClick.find(before,new JSONArray().put(after)));
        after=node("line");GpuPlot.plot(after).put("values",new JSONArray("[10,5,10]"));
        assertEquals(-1,GpuClick.find(before,new JSONArray().put(after)));
    }


}
