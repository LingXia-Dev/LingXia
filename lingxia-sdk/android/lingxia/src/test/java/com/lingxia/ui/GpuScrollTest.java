package com.lingxia.ui;

import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuScrollTest {
    @Test public void orthogonalGesturesDoNotTransferDistanceAcrossAxes() throws Exception {
        JSONObject frame = frame(0, 50);
        frame.getJSONArray("nodes").getJSONObject(1).getJSONObject("scroll").put("axis", "horizontal");
        GpuScroll scroll = new GpuScroll();
        scroll.accept(frame, 0, scroll.snapshot());
        assertEquals("inner", scroll.targetForAxis("inner", 0));
        assertEquals("outer", scroll.targetForAxis("inner", 1));
        assertTrue(scroll.scrollBy("inner", 75));
        assertEquals(50, scroll.offset("inner"), 0);
        assertEquals(0, scroll.offset("outer"), 0);
        assertFalse(scroll.canScroll("inner", 1));
        assertTrue(scroll.canScroll("inner", 1, 1));
        assertFalse(scroll.canScroll("inner", -1, 1));
    }

    @Test public void horizontalRevealUsesWidthAndRejectsUnrecoverableClipsAtomically() throws Exception {
        JSONObject frame = new JSONObject("{nodes:[{bounds:[0,0,100,100],visible_bounds:[0,0,100,100],scroll:{id:'outer',parent:null,offset:0,max_offset:200}},"
            + "{bounds:[0,20,100,60],visible_bounds:[0,20,100,60],scroll:{axis:'horizontal',id:'inner',parent:'outer',offset:0,max_offset:200}}]}");
        GpuScroll scroll = new GpuScroll(); scroll.accept(frame, 0, scroll.snapshot());
        JSONObject target = new JSONObject("{bounds:[220,30,40,40],scroll_parent:'inner'}");
        assertTrue(scroll.canReveal(frame, target));
        assertTrue(scroll.reveal(frame, target));
        assertEquals(160, scroll.offset("inner"), 0);
        assertEquals(0, scroll.offset("outer"), 0);
        assertFalse(scroll.reveal(frame, target));
        long revision = scroll.revision;
        assertFalse(scroll.reveal(frame, new JSONObject("{bounds:[400,30,40,40],scroll_parent:'inner'}")));
        assertEquals(160, scroll.offset("inner"), 0);
        assertEquals(revision, scroll.revision);
    }
    private JSONObject frame(double offset, double max) throws Exception {
        return new JSONObject("{nodes:[{scroll:{id:'outer',parent:null,offset:0,max_offset:200}},"
            + "{scroll:{id:'inner',parent:'outer',offset:" + offset + ",max_offset:" + max + "}}]}");
    }
    @Test public void nestedBoundaryTransfersOnlyRemainingDistance() throws Exception {
        GpuScroll scroll = new GpuScroll();
        scroll.accept(frame(0, 50), 0, scroll.snapshot());
        assertTrue(scroll.scrollBy("inner", 75));
        assertEquals(50, scroll.offset("inner"), 0);
        assertEquals(25, scroll.offset("outer"), 0);
        assertTrue(scroll.scrollBy("inner", -100));
        assertEquals(0, scroll.offset("inner"), 0);
        assertEquals(0, scroll.offset("outer"), 0);
        assertFalse(scroll.scrollBy("inner", -1));
        assertFalse(scroll.scrollBy("inner", Double.NaN));
    }
    @Test public void refreshBoundaryIncludesParentsAndPendingFingerOffsets() throws Exception {
        GpuScroll scroll = new GpuScroll();
        scroll.accept(frame(0, 50), 0, scroll.snapshot());
        assertFalse(scroll.canScroll("inner", -1));
        assertTrue(scroll.canScroll("inner", 1));
        scroll.scrollBy("outer", 30);
        // The touched inner region is at zero, but its parent still owns the downward drag.
        assertTrue(scroll.canScroll("inner", -1));
        long captured = scroll.revision;
        java.util.Map<String, Double> offsets = scroll.snapshot();
        scroll.scrollBy("inner", -30);
        assertFalse(scroll.canScroll("inner", -1));
        JSONObject late = frame(0, 50);
        late.getJSONArray("nodes").getJSONObject(0).getJSONObject("scroll").put("offset", 30);
        scroll.accept(late, captured, offsets);
        assertFalse(scroll.canScroll("inner", -1));
        scroll.scrollBy("inner", 300);
        assertFalse(scroll.canScroll("inner", 1));
        assertTrue(scroll.canScroll("inner", -1));
        assertFalse(scroll.canScroll(null, -1));
        assertFalse(scroll.canScroll("missing", -1));
    }
    @Test public void lateFramePreservesNewerInputAndRemovedRegionsArePruned() throws Exception {
        GpuScroll scroll = new GpuScroll();
        scroll.accept(frame(0, 100), 0, scroll.snapshot());
        scroll.scrollBy("inner", 25);
        long captured = scroll.revision;
        java.util.Map<String, Double> capturedOffsets = scroll.snapshot();
        scroll.scrollBy("inner", 30);
        scroll.accept(frame(25, 100), captured, capturedOffsets);
        assertEquals(55, scroll.offset("inner"), 0);
        scroll.accept(frame(55, 40), captured, capturedOffsets);
        assertEquals(40, scroll.offset("inner"), 0);
        scroll.accept(new JSONObject("{nodes:[]}"), scroll.revision, scroll.snapshot());
        assertEquals("{}", scroll.toJson());
        assertFalse(scroll.scrollBy("inner", 30));
    }
    @Test public void measuredOriginCorrectionPreservesPendingFingerDelta() throws Exception {
        GpuScroll scroll = new GpuScroll();
        scroll.accept(frame(0, 200), 0, scroll.snapshot());
        scroll.scrollBy("inner", 25);
        long captured = scroll.revision;
        java.util.Map<String, Double> capturedOffsets = scroll.snapshot();
        scroll.scrollBy("inner", 30);
        scroll.accept(frame(75, 200), captured, capturedOffsets);
        assertEquals(105, scroll.offset("inner"), 0);
        assertEquals(25, capturedOffsets.get("inner"), 0);
    }

    @Test public void revealMovesNestedAncestorsAndDoesNotDoubleApplyPendingOffsets()throws Exception{
        JSONObject frame=new JSONObject("{nodes:[{bounds:[0,0,100,100],visible_bounds:[0,0,100,100],scroll:{id:'outer',parent:null,offset:0,max_offset:200}},"
            +"{bounds:[0,120,100,60],visible_bounds:[0,120,100,0],scroll:{id:'inner',parent:'outer',offset:0,max_offset:200}}]}");
        JSONObject target=new JSONObject("{bounds:[0,320,100,40],scroll_parent:'inner'}");
        GpuScroll scroll=new GpuScroll();scroll.accept(frame,0,scroll.snapshot());
        assertTrue(scroll.reveal(frame,target));assertEquals(180,scroll.offset("inner"),0);assertEquals(80,scroll.offset("outer"),0);
        long revision=scroll.revision;assertFalse(scroll.reveal(frame,target));assertEquals(revision,scroll.revision);
    }
    @Test public void revealAlignsOversizedTargetsAndRejectsUnreachableOrCyclicRegions()throws Exception{
        JSONObject frame=new JSONObject("{nodes:[{bounds:[0,0,100,100],visible_bounds:[0,0,100,100],scroll:{id:'s',parent:null,offset:0,max_offset:200}}]}");
        GpuScroll scroll=new GpuScroll();scroll.accept(frame,0,scroll.snapshot());
        assertTrue(scroll.reveal(frame,new JSONObject("{bounds:[0,30,100,160],scroll_parent:'s'}")));assertEquals(30,scroll.offset("s"),0);
        assertFalse(scroll.canReveal(frame,new JSONObject("{bounds:[0,200,40,40],scroll_parent:'missing'}")));
        frame.getJSONArray("nodes").getJSONObject(0).getJSONObject("scroll").put("parent","s");assertFalse(scroll.canReveal(frame,new JSONObject("{bounds:[0,200,40,40],scroll_parent:'s'}")));
    }
    @Test public void revealRequiresHorizontalIntersectionThroughEveryAncestor()throws Exception {
        JSONObject frame=new JSONObject("{nodes:[{bounds:[0,0,100,100],visible_bounds:[20,0,80,100],scroll:{id:'outer',parent:null,offset:0,max_offset:200}},"
            +"{bounds:[0,20,60,60],visible_bounds:[20,20,40,60],scroll:{id:'inner',parent:'outer',offset:0,max_offset:200}}]}");
        GpuScroll scroll=new GpuScroll();scroll.accept(frame,0,scroll.snapshot());
        for(String box:new String[]{"[120,200,40,40]","[70,200,40,40]","[0,200,20,40]","[20,200,0,40]"}) {
            JSONObject target=new JSONObject("{bounds:"+box+",scroll_parent:'inner'}");
            assertFalse(scroll.canReveal(frame,target));assertFalse(scroll.reveal(frame,target));
            assertEquals(0,scroll.offset("inner"),0);assertEquals(0,scroll.offset("outer"),0);
        }
        JSONObject partial=new JSONObject("{bounds:[10,200,20,40],scroll_parent:'inner'}");
        assertTrue(scroll.canReveal(frame,partial));assertTrue(scroll.reveal(frame,partial));
    }

}
