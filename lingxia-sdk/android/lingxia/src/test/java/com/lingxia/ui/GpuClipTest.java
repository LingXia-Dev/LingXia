package com.lingxia.ui;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuClipTest {
    @Test public void independentOwnCornersMatchTouchesAndHighlight() throws Exception {
        JSONObject node=new JSONObject("{bounds:[0,0,100,80],visible_bounds:[0,0,100,80],radius:24,radii:[[24,24],[24,24],[6,6],[24,24]]}");
        assertFalse(GpuClip.contains(node,98,2,true));
        assertTrue(GpuClip.contains(node,98,78,true));
        GpuClip.Highlight highlight=new GpuClip.Highlight();highlight.setNode(node,2);
        assertArrayEquals(new float[]{48,48,48,48,12,12,48,48},highlight.getCornerRadii(),0);
        node.remove("radii");highlight.setNode(node,2);
        assertArrayEquals(new float[]{48,48,48,48,48,48,48,48},highlight.getCornerRadii(),0);
    }
    @Test public void invisibleRoundedCornersCannotReceiveChildTouches() throws Exception {
        JSONObject node = new JSONObject("{bounds:[0,0,100,200],visible_bounds:[0,0,100,100],clips:[{bounds:[0,0,100,100],radius:30}]}");
        assertFalse(GpuClip.contains(node,1,1,false));
        assertFalse(GpuClip.contains(node,99,99,false));
        assertTrue(GpuClip.contains(node,50,1,false));
        assertTrue(GpuClip.contains(node,1,50,false));
        assertFalse(GpuClip.contains(node,50,150,false));
    }
    @Test public void nestedMasksUseUnclippedGeometryAndAllAncestors() throws Exception {
        JSONObject node = new JSONObject("{bounds:[0,-20,100,200],visible_bounds:[0,0,100,60],clips:[{bounds:[0,-20,100,80],radius:30},{bounds:[0,0,100,100],radius:10}]}");
        assertTrue(GpuClip.contains(node,10,1,false));
        assertFalse(GpuClip.contains(node,1,1,false));
        assertFalse(GpuClip.contains(node,1,59,false));
    }
    @Test public void scrollHitAlsoObeysItsOwnRadiusAndCapsOversizedRadii() throws Exception {
        JSONObject node = new JSONObject("{bounds:[10,20,100,40],visible_bounds:[10,20,100,40],radius:1000}");
        assertFalse(GpuClip.contains(node,11,21,true));
        assertTrue(GpuClip.contains(node,60,21,true));
        assertTrue(GpuClip.contains(node,11,40,true));
        assertTrue(GpuClip.contains(node,11,21,false));
    }
    @Test public void highlightRetainsOriginalBoundsWhileMaskingTheVisibleRegion() throws Exception {
        GpuClip.Highlight highlight = new GpuClip.Highlight();
        highlight.setNode(new JSONObject("{bounds:[10,-20,100,100],visible_bounds:[10,0,100,60],clips:[{bounds:[10,-20,100,80],radius:30}]}"),2);
        assertEquals(new android.graphics.Rect(20,-40,220,160),highlight.getBounds());
    }
    @Test @Config(sdk=31) @org.robolectric.annotation.GraphicsMode(org.robolectric.annotation.GraphicsMode.Mode.NATIVE)
    public void asymmetricInnerCornersMaskTouchesAndHighlights() throws Exception {
        JSONObject node = new JSONObject("{bounds:[0,0,100,80],visible_bounds:[8,2,88,72],clips:[{bounds:[8,2,88,72],radius:0,radii:[[12,18],[16,18],[16,14],[12,14]]}]}");
        assertFalse(GpuClip.contains(node,9,3,false));
        assertTrue(GpuClip.contains(node,20,3,false));
        assertTrue(GpuClip.contains(node,9,20,false));
        assertFalse(GpuClip.contains(node,95,73,false));
        assertTrue(GpuClip.contains(node,80,73,false));
        GpuClip.Highlight highlight = new GpuClip.Highlight();
        highlight.setNode(node,2);
        java.lang.reflect.Field masks=GpuClip.Highlight.class.getDeclaredField("masks");masks.setAccessible(true);
        android.graphics.Path[] paths=(android.graphics.Path[])masks.get(highlight);
        android.graphics.Region region=new android.graphics.Region();
        region.setPath(paths[1],new android.graphics.Region(0,0,200,160));
        assertFalse(region.contains(18,6));
        assertTrue(region.contains(40,6));
        assertTrue(region.contains(18,40));
    }
    @Test public void malformedInnerRadiiCannotReceiveTouches() throws Exception {
        for(String radii:new String[]{"[]","[[12,18]]","[[-1,2],[0,0],[0,0],[0,0]]"}) {
            JSONObject node=new JSONObject("{bounds:[0,0,100,80],clips:[{bounds:[0,0,100,80],radii:"+radii+"}]}");
            assertFalse(GpuClip.contains(node,50,40,false));
        }
    }
}
