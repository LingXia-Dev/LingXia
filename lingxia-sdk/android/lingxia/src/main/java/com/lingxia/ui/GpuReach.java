package com.lingxia.ui;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/** Edge transitions of presented frames. Preparation alone must never deliver events. */
final class GpuReach {
    private final Map<String, boolean[]> entered = new HashMap<>();
    boolean pending;

    void clear() { entered.clear(); pending = false; }

    List<Long> presented(JSONObject frame) {
        List<Long> events = new ArrayList<>();
        Set<String> alive = new HashSet<>();
        pending = false;
        JSONArray nodes = frame.optJSONArray("nodes");
        for (int i = 0; nodes != null && i < nodes.length(); i++) {
            JSONObject node = nodes.optJSONObject(i);
            JSONObject scroll = node.optJSONObject("scroll");
            if (scroll == null) continue;
            String id = scroll.optString("id");
            alive.add(id);
            boolean[] state = entered.computeIfAbsent(id, ignored -> new boolean[2]);
            JSONArray bounds = node.optJSONArray("visible_bounds");
            boolean visible = bounds != null && bounds.optDouble(2) > 0 && bounds.optDouble(3) > 0;
            double offset = scroll.optDouble("offset"), max = scroll.optDouble("max_offset");
            for (int edge = 0; edge < 2; edge++) {
                long handle = scroll.optLong(edge == 0 ? "on_reach_start" : "on_reach_end");
                boolean atEdge = edge == 0 ? offset <= 0.01 : offset >= max - 0.01;
                if (!atEdge || handle == 0) { state[edge] = false; continue; }
                if (state[edge] || !visible) continue;
                // Bound one worker batch; remaining visible edges get a later presented frame.
                if (events.size() == 32) { pending = true; continue; }
                events.add(handle);
                state[edge] = true;
            }
        }
        entered.keySet().retainAll(alive);
        return events;
    }
}
