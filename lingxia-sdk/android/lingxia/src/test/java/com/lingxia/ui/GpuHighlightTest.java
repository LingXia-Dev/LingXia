package com.lingxia.ui;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuHighlightTest {
    @org.junit.Before public void loadCore() { GpuNative.load(); }

    @Test public void independentOwnCornersMatchTouchesAndHighlight() throws Exception {
        JSONObject node=new JSONObject("{bounds:[0,0,100,80],visible_bounds:[0,0,100,80],radius:24,radii:[[24,24],[24,24],[6,6],[24,24]]}");
        // The shared core answers hit geometry; this checks the JNI wiring.
        assertFalse(GpuHighlight.contains(node,98,2,true));
        assertTrue(GpuHighlight.contains(node,98,78,true));
        GpuHighlight.Highlight highlight=new GpuHighlight.Highlight();highlight.setNode(node,2);
        assertArrayEquals(new float[]{48,48,48,48,12,12,48,48},highlight.getCornerRadii(),0);
        node.remove("radii");highlight.setNode(node,2);
        assertArrayEquals(new float[]{48,48,48,48,48,48,48,48},highlight.getCornerRadii(),0);
    }
    @Test public void highlightRetainsOriginalBoundsWhileMaskingTheVisibleRegion() throws Exception {
        GpuHighlight.Highlight highlight = new GpuHighlight.Highlight();
        highlight.setNode(new JSONObject("{bounds:[10,-20,100,100],visible_bounds:[10,0,100,60],clips:[{bounds:[10,-20,100,80],radius:30}]}"),2);
        assertEquals(new android.graphics.Rect(20,-40,220,160),highlight.getBounds());
    }
    @Test @Config(sdk=31) @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    public void asymmetricInnerCornersMaskHighlights() throws Exception {
        JSONObject node = new JSONObject("{bounds:[0,0,100,80],visible_bounds:[8,2,88,72],clips:[{bounds:[8,2,88,72],radius:0,radii:[[12,18],[16,18],[16,14],[12,14]]}]}");
        GpuHighlight.Highlight highlight = new GpuHighlight.Highlight();
        highlight.setNode(node,2);
        java.lang.reflect.Field masks=GpuHighlight.Highlight.class.getDeclaredField("masks");masks.setAccessible(true);
        android.graphics.Path[] paths=(android.graphics.Path[])masks.get(highlight);
        android.graphics.Region region=new android.graphics.Region();
        region.setPath(paths[1],new android.graphics.Region(0,0,200,160));
        assertFalse(region.contains(18,6));
        assertTrue(region.contains(40,6));
        assertTrue(region.contains(18,40));
    }

}
