package com.lingxia.ui;

import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** One in-flight proposal plus the latest desired value; identities survive presented frames. */
final class GpuSlider {
    static final int SET_PROGRESS=16908349;
    static final class Proposal {
        final String identity, definition;
        final double value;
        final long page, epoch;
        Proposal(JSONObject node,double value,long page,long epoch) {
            identity=node.optString("identity");definition=definition(node);this.value=value;this.page=page;this.epoch=epoch;
        }
        boolean matches(JSONObject node) { return enabled(node) && identity.equals(node.optString("identity")) && definition.equals(definition(node)); }
    }
    volatile long epoch;
    Proposal pending, flight;
    private JSONObject gesture;
    private float downX,downY,scale;
    private boolean claimed;

    static JSONObject range(JSONObject node) {return node==null?null:node.optJSONObject("range");}
    static boolean enabled(JSONObject node) {JSONObject r=range(node);return r!=null && r.optBoolean("enabled") && r.optLong("on_change")>0;}
    static String definition(JSONObject node) {JSONObject r=range(node);return r==null?"":r.optDouble("min")+":"+r.optDouble("max")+":"+r.optDouble("step");}
    static double normalize(JSONObject node,double value) {
        JSONObject r=range(node);double min=r.optDouble("min"),max=r.optDouble("max"),step=r.optDouble("step");
        value=Math.max(min,Math.min(max,value));
        double snapped=Math.max(min,Math.min(max,min+Math.floor((value-min)/step+0.5)*step));
        return Math.abs(max-value)<Math.abs(snapped-value)?max:snapped;
    }
    static JSONArray contentBounds(JSONObject node) {
        JSONArray bounds=node.optJSONArray("control_bounds");return bounds==null?node.optJSONArray("bounds"):bounds;
    }
    static double at(JSONObject node,float x) {
        JSONObject r=range(node);JSONArray box=contentBounds(node);double inset=r.optDouble("inset");
        double width=box.optDouble(2)-2*inset;
        return width<=0?r.optDouble("value"):normalize(node,r.optDouble("min")+(r.optDouble("max")-r.optDouble("min"))*(x-box.optDouble(0)-inset)/width);
    }
    static double shift(JSONObject node,double value,int direction) {
        JSONObject r=range(node);double min=r.optDouble("min"),max=r.optDouble("max"),step=r.optDouble("step");
        if(direction<0 && value==max) {
            double last=min+Math.floor((max-min)/step)*step;
            if(last<max)return normalize(node,last);
        }
        return normalize(node,value+direction*step);
    }
    double value(JSONObject node) {
        if(pending!=null && pending.matches(node))return pending.value;
        if(flight!=null && flight.epoch==epoch && flight.matches(node))return flight.value;
        return range(node).optDouble("value");
    }
    boolean propose(JSONObject node,double value,long page) {
        if(!enabled(node) || !Double.isFinite(value) || Math.abs(value)>1e9)return false;
        pending=new Proposal(node,normalize(node,value),page,epoch);return true;
    }
    void cancel() {epoch++;pending=null;gesture=null;claimed=false;}
    boolean begin(JSONObject node,float x,float y,float scale) {
        if(!enabled(node))return false;
        gesture=node;downX=x;downY=y;this.scale=scale;claimed=false;return true;
    }
    boolean hasGesture() {return gesture!=null;}
    JSONObject gesture() {return gesture;}
    boolean isDragging() { return claimed; }
    boolean move(float x,float y,float slop) {
        if(gesture==null)return false;
        if(!claimed && Math.abs(y-downY)>slop && Math.abs(y-downY)>Math.abs(x-downX)){cancel();return false;}
        if(Math.abs(x-downX)>slop && Math.abs(x-downX)>=Math.abs(y-downY))claimed=true;
        return claimed;
    }
    void end() {gesture=null;claimed=false;}
    void presented(JSONObject frame,float scale) {
        if(gesture==null)return;
        JSONArray nodes=frame.optJSONArray("nodes");JSONObject current=null;
        for(int i=0;nodes!=null && i<nodes.length();i++)if(gesture.optString("identity").equals(nodes.optJSONObject(i).optString("identity"))){current=nodes.optJSONObject(i);break;}
        JSONArray visible=current==null?null:current.optJSONArray("visible_bounds");
        if(!enabled(current) || (visible!=null && (visible.optDouble(2)<=0 || visible.optDouble(3)<=0)) || this.scale!=scale || !definition(gesture).equals(definition(current))
            || !gesture.optJSONArray("bounds").toString().equals(current.optJSONArray("bounds").toString())
            || !contentBounds(gesture).toString().equals(contentBounds(current).toString()))cancel();
        else gesture=current;
    }
    static java.util.List<Integer> changed(JSONObject previous,JSONObject next) {
        java.util.Map<String,Double> values=new java.util.HashMap<>();
        JSONArray before=previous==null?null:previous.optJSONArray("nodes"),after=next.optJSONArray("nodes");
        for(int i=0;before!=null && i<before.length();i++) {
            JSONObject node=before.optJSONObject(i);if(range(node)!=null)values.put(node.optString("identity"),range(node).optDouble("value"));
        }
        java.util.List<Integer> changed=new java.util.ArrayList<>();
        for(int i=0;after!=null && i<after.length();i++) {
            JSONObject node=after.optJSONObject(i);Double old=values.get(node.optString("identity"));
            if(old!=null && range(node)!=null && old.doubleValue()!=range(node).optDouble("value"))changed.add(i);
        }
        return changed;
    }
    static void populate(AccessibilityNodeInfo info,JSONObject node) {
        JSONObject r=range(node);if(r==null)return;
        info.setClassName("android.widget.SeekBar");info.setContentDescription(node.optString("accessibility_label"));
        info.setEnabled(enabled(node));info.setFocusable(true);info.setClickable(false);
        info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,(float)r.optDouble("min"),(float)r.optDouble("max"),(float)r.optDouble("value")));
        if(enabled(node)) {
            if(android.os.Build.VERSION.SDK_INT>=24)info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);
            if(r.optDouble("value")<r.optDouble("max"))info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            if(r.optDouble("value")>r.optDouble("min"))info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
        }
    }
}
