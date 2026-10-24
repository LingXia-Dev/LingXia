package com.lingxia.ui;

import java.util.Arrays;
import java.util.Collections;
import org.json.JSONArray;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;
import static org.junit.Assert.*;

@RunWith(RobolectricTestRunner.class)
@Config(manifest = Config.NONE, sdk = 28)
public class GpuReachTest {
    private JSONObject frame(double offset, double max, boolean visible, long start, long end) throws Exception {
        return new JSONObject("{nodes:[{visible_bounds:[0,0,100," + (visible ? 100 : 0)
            + "],scroll:{id:'feed',offset:" + offset + ",max_offset:" + max
            + ",on_reach_start:" + start + ",on_reach_end:" + end + "}}]}");
    }

    @Test public void edgesFireOnceUntilLeavingRegardlessOfNewHandlerHandles() throws Exception {
        GpuReach reach = new GpuReach();
        assertEquals(Arrays.asList(11L), reach.presented(frame(0, 100, true, 11, 12)));
        assertTrue(reach.presented(frame(0, 100, true, 21, 22)).isEmpty());
        assertTrue(reach.presented(frame(30, 100, true, 21, 22)).isEmpty());
        assertEquals(Arrays.asList(22L), reach.presented(frame(100, 100, true, 21, 22)));
        assertTrue(reach.presented(frame(100, 100, true, 31, 32)).isEmpty());
        assertEquals(Arrays.asList(31L), reach.presented(frame(0, 100, true, 31, 32)));
    }

    @Test public void hiddenRegionsDoNotEmitAndUnmountOrClearResetsEdges() throws Exception {
        GpuReach reach = new GpuReach();
        assertTrue(reach.presented(frame(0, 0, false, 1, 2)).isEmpty());
        assertEquals(Arrays.asList(1L, 2L), reach.presented(frame(0, 0, true, 1, 2)));
        assertTrue(reach.presented(frame(0, 0, false, 1, 2)).isEmpty());
        assertTrue(reach.presented(frame(0, 0, true, 1, 2)).isEmpty());
        reach.presented(new JSONObject("{nodes:[]}"));
        assertEquals(Arrays.asList(1L, 2L), reach.presented(frame(0, 0, true, 1, 2)));
        reach.clear();
        assertEquals(Arrays.asList(1L, 2L), reach.presented(frame(0, 0, true, 1, 2)));
    }

    @Test public void subscribingAtAnEdgeAndGrowingContentHaveDistinctTransitions() throws Exception {
        GpuReach reach = new GpuReach();
        assertTrue(reach.presented(frame(100, 100, true, 0, 0)).isEmpty());
        assertEquals(Arrays.asList(2L), reach.presented(frame(100, 100, true, 0, 2)));
        assertTrue(reach.presented(frame(100, 200, true, 0, 3)).isEmpty());
        assertEquals(Arrays.asList(3L), reach.presented(frame(200, 200, true, 0, 3)));
    }

    @Test public void largeEdgeSetsDrainInBoundedBatchesWithoutDroppingCallbacks() throws Exception {
        GpuReach reach = new GpuReach();
        JSONArray nodes = new JSONArray();
        for (int i = 0; i < 40; i++) {
            JSONObject node = frame(0, 0, true, 2*i+1, 2*i+2).getJSONArray("nodes").getJSONObject(0);
            node.getJSONObject("scroll").put("id", "feed-"+i); nodes.put(node);
        }
        JSONObject frame = new JSONObject().put("nodes", nodes);
        java.util.Set<Long> delivered = new java.util.HashSet<>();
        assertEquals(32, reach.presented(frame).stream().filter(delivered::add).count());
        assertTrue(reach.pending);
        assertEquals(32, reach.presented(frame).stream().filter(delivered::add).count());
        assertEquals(16, reach.presented(frame).stream().filter(delivered::add).count());
        assertFalse(reach.pending);
        assertEquals(80, delivered.size());
        assertEquals(Collections.emptyList(), reach.presented(frame));
    }
}
