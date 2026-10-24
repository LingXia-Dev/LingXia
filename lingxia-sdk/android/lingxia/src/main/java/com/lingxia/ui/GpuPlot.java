package com.lingxia.ui;

import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** Android accessibility actions; selection geometry is resolved by the Rust session. */
final class GpuPlot {
    static final int SELECT_ITEM = 0x02010000;
    static JSONObject plot(JSONObject node) { return node == null ? null : node.optJSONObject("plot"); }
    static boolean enabled(JSONObject node) {
        JSONObject plot = plot(node);
        return plot != null && plot.optLong("on_select") > 0;
    }
    static int count(JSONObject node) { return plot(node).optJSONArray("values").length(); }
    static boolean canClear(JSONObject node) {
        return enabled(node) && plot(node).optInt("selected_index", -1) >= 0;
    }
    static java.util.List<JSONObject> outsideSelections(JSONArray nodes,float x,float y) {
        java.util.List<JSONObject> result=new java.util.ArrayList<>();
        if(!Float.isFinite(x)||!Float.isFinite(y))return result;
        for(int i=0;nodes!=null&&i<nodes.length();i++){
            JSONObject node=nodes.optJSONObject(i);
            if(!canClear(node))continue;
            JSONArray visible=node.optJSONArray("visible_bounds");
            if(visible==null)visible=node.optJSONArray("bounds");
            if(visible==null||visible.optDouble(2)<=0||visible.optDouble(3)<=0)continue;
            if(!GpuClip.contains(node,x,y,false)||!GpuClip.containsContent(node,x,y))result.add(node);
        }
        return result;
    }
    static boolean selectable(JSONObject node, int index) {
        JSONObject plot = plot(node);
        return enabled(node) && index >= 0 && index < count(node)
            && (!"pie".equals(plot.optString("kind")) || plot.optJSONArray("values").optDouble(index) > 0);
    }
    static void populate(AccessibilityNodeInfo info, JSONObject node) {
        if (!enabled(node)) return;
        info.setFocusable(true);
        info.setClickable(true);
        if (canClear(node)) info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_SELECTION);
        for (int i = 0; i < count(node); i++) if (selectable(node, i))
            info.addAction(new AccessibilityNodeInfo.AccessibilityAction(SELECT_ITEM + i,
                "Select item " + (i + 1) + ": " + plot(node).optJSONArray("values").optDouble(i)));
    }
}
