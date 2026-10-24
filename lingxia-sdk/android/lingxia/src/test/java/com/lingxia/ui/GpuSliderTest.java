package com.lingxia.ui;

import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import android.view.accessibility.AccessibilityNodeInfo;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest=Config.NONE,sdk={23,31})
public class GpuSliderTest {
    static JSONObject node()throws Exception{return new JSONObject("{identity:'volume',tag:'slider',accessibility_label:'Volume',bounds:[10,20,200,48],visible_bounds:[10,20,200,48],range:{min:0,max:10,step:3,value:3,inset:24,enabled:true,on_change:7}}");}
    static JSONObject frame(JSONObject node)throws Exception{return new JSONObject().put("nodes",new JSONArray().put(node));}
    @Test public void snappingAndKeysReachBothEndpointsAndThePartialFinalStep()throws Exception{
        JSONObject node=node();assertEquals(0,GpuSlider.at(node,0),0);assertEquals(10,GpuSlider.at(node,300),0);
        assertEquals(3,GpuSlider.normalize(node,4),0);assertEquals(10,GpuSlider.normalize(node,9.6),0);
        assertEquals(9,GpuSlider.shift(node,10,-1),0);assertEquals(6,GpuSlider.shift(node,9,-1),0);assertEquals(10,GpuSlider.shift(node,9,1),0);
        assertEquals(10,GpuSlider.shift(node,10,1),0);assertEquals(0,GpuSlider.shift(node,0,-1),0);
        node.getJSONObject("range").put("min",-1).put("max",1).put("step",0.25);assertEquals(-0.5,GpuSlider.normalize(node,-0.6),0);
    }
    @Test public void pendingValuesCoalesceAndKeyboardStepsUseTheLatestDesiredValue()throws Exception{
        JSONObject node=node();GpuSlider slider=new GpuSlider();
        assertTrue(slider.propose(node,6,4));slider.flight=slider.pending;slider.pending=null;
        assertEquals(6,slider.value(node),0);assertTrue(slider.propose(node,9,4));assertTrue(slider.propose(node,10,4));assertEquals(10,slider.value(node),0);
        assertFalse(slider.propose(node,Double.NaN,4));assertEquals(10,slider.pending.value,0);
        node.getJSONObject("range").put("on_change",8);assertTrue(slider.pending.matches(node));
        node.getJSONObject("range").put("step",2);assertFalse(slider.pending.matches(node));
        long epoch=slider.epoch;slider.cancel();assertTrue(slider.epoch>epoch);assertNull(slider.pending);assertEquals(3,slider.value(node),0);
        node.getJSONObject("range").put("enabled",false);assertFalse(slider.propose(node,3,4));
    }
    @Test public void horizontalGestureSurvivesValueFramesButYieldsVerticalMotionAndResizes()throws Exception{
        JSONObject node=node();GpuSlider slider=new GpuSlider();assertTrue(slider.begin(node,50,40,2));
        assertFalse(slider.move(52,41,8));assertTrue(slider.move(80,43,8));assertTrue(slider.hasGesture());
        JSONObject next=node();next.getJSONObject("range").put("value",9).put("on_change",8);slider.presented(frame(next),2);assertTrue(slider.hasGesture());
        JSONObject resized=new JSONObject(next.toString()).put("bounds",new JSONArray("[10,20,300,48]"));slider.presented(frame(resized),2);assertFalse(slider.hasGesture());
        slider.begin(node,50,40,2);assertFalse(slider.move(52,70,8));assertFalse(slider.hasGesture());
        slider.begin(node,50,40,2);slider.presented(frame(node),3);assertFalse(slider.hasGesture());
        slider.begin(node,50,40,2);slider.presented(new JSONObject("{nodes:[]}"),2);assertFalse(slider.hasGesture());
    }
    @Test public void accessibilityExposesRangeLabelAndOnlyEnabledAdjustments()throws Exception{
        JSONObject node=node();AccessibilityNodeInfo info=AccessibilityNodeInfo.obtain();GpuSlider.populate(info,node);
        assertEquals("android.widget.SeekBar",info.getClassName());assertEquals("Volume",info.getContentDescription());assertEquals(3,info.getRangeInfo().getCurrent(),0);
        assertTrue(info.isEnabled());assertTrue(info.isFocusable());assertFalse(info.isClickable());assertTrue((info.getActions() & AccessibilityNodeInfo.ACTION_SCROLL_FORWARD)!=0);
        if(android.os.Build.VERSION.SDK_INT>=24)assertTrue(info.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS));
        info.recycle();node.getJSONObject("range").put("enabled",false);info=AccessibilityNodeInfo.obtain();GpuSlider.populate(info,node);assertFalse(info.isEnabled());assertEquals(0,info.getActions());info.recycle();
    }
    @Test public void changedValuesFollowIdentityAndIgnoreReorderingAndNewNodes()throws Exception{
        JSONObject before=frame(node()),same=frame(node());assertTrue(GpuSlider.changed(before,same).isEmpty());
        JSONObject changed=node();changed.getJSONObject("range").put("value",9);
        JSONObject after=new JSONObject().put("nodes",new JSONArray().put(new JSONObject("{identity:'label'}")).put(changed));
        assertEquals(java.util.Arrays.asList(1),GpuSlider.changed(before,after));assertTrue(GpuSlider.changed(null,after).isEmpty());
    }
    @Test public void borderedTrackUsesContentBoundsAndCancelsChangedGeometry() throws Exception {
        JSONObject node=node();node.put("control_bounds",new JSONArray("[30,25,120,38]"));
        assertEquals(0,GpuSlider.at(node,54),0);assertEquals(10,GpuSlider.at(node,126),0);
        GpuSlider slider=new GpuSlider();assertTrue(slider.begin(node,70,40,1));
        JSONObject changed=new JSONObject(node.toString());changed.put("control_bounds",new JSONArray("[35,25,110,38]"));
        slider.presented(frame(changed),1);assertFalse(slider.hasGesture());
    }
}
