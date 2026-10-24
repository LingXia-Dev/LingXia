package com.lingxia.ui;

import java.util.HashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/** UI-thread desired offsets. A late frame cannot overwrite newer finger movement. */
final class GpuScroll {
    private final Map<String, Double> offsets = new HashMap<>();
    private final Map<String, JSONObject> regions = new HashMap<>();
    long revision;

    void clear() { offsets.clear(); regions.clear(); revision++; }
    Map<String, Double> snapshot() { return new HashMap<>(offsets); }
    String toJson() { return new JSONObject(offsets).toString(); }
    double offset(String id) { return offsets.getOrDefault(id, 0.0); }

    static int axis(JSONObject region) { return region != null && "horizontal".equals(region.optString("axis")) ? 0 : 1; }
    int axis(String id) { return axis(regions.get(id)); }

    String targetForAxis(String id, int requestedAxis) {
        for (int depth = 0; id != null && depth < 129; depth++) {
            JSONObject region = regions.get(id);
            if (region == null) break;
            if (axis(region) == requestedAxis && region.optDouble("max_offset") > 0) return id;
            id = parent(region);
        }
        return null;
    }

    void accept(JSONObject frame, long capturedRevision, Map<String, Double> capturedOffsets) {
        regions.clear();
        JSONArray nodes = frame.optJSONArray("nodes");
        for (int i = 0; nodes != null && i < nodes.length(); i++) {
            JSONObject scroll = nodes.optJSONObject(i).optJSONObject("scroll");
            if (scroll == null) continue;
            String id = scroll.optString("id");
            regions.put(id, scroll);
            double correction = scroll.optDouble("offset") - capturedOffsets.getOrDefault(id, 0.0);
            // Measurement can move the frame's origin while newer finger deltas are pending.
            double desired = capturedRevision == revision ? scroll.optDouble("offset") : offset(id) + correction;
            offsets.put(id, Math.max(0, Math.min(scroll.optDouble("max_offset"), desired)));
        }
        offsets.keySet().retainAll(regions.keySet());
    }

    private static String parent(JSONObject region) { return region.isNull("parent") ? null : region.optString("parent",null); }

    private static boolean validBox(JSONArray box) {
        if(box==null || box.length()!=4)return false;
        for(int i=0;i<4;i++)if(!Double.isFinite(box.optDouble(i)))return false;
        return box.optDouble(2)>0 && box.optDouble(3)>0;
    }

    private Map<String, Double> revealOffsets(JSONObject frame, JSONObject target) {
        JSONArray nodes = frame.optJSONArray("nodes"), targetBox = target.optJSONArray("bounds");
        String id = target.optString("scroll_parent", null);
        if (nodes == null || id == null || !validBox(targetBox)) return null;
        double[] start = {targetBox.optDouble(0), targetBox.optDouble(1)};
        double[] end = {start[0] + targetBox.optDouble(2), start[1] + targetBox.optDouble(3)};
        Map<String, Double> changes = new HashMap<>();
        for (int depth = 0; id != null && depth < 129; depth++) {
            JSONObject owner = null;
            for (int i = 0; i < nodes.length(); i++) {
                JSONObject node = nodes.optJSONObject(i), region = node.optJSONObject("scroll");
                if (region != null && id.equals(region.optString("id"))) { owner = node; break; }
            }
            if (owner == null) return null;
            JSONObject region = owner.optJSONObject("scroll");
            String parent = parent(region);
            JSONArray box = owner.optJSONArray(parent == null ? "visible_bounds" : "bounds");
            if (!validBox(box)) return null;
            int axis = axis(region), cross = 1 - axis;
            start[cross] = Math.max(start[cross], box.optDouble(cross));
            end[cross] = Math.min(end[cross], box.optDouble(cross) + box.optDouble(cross + 2));
            if (end[cross] <= start[cross]) return null;
            double before = offset(id), pending = before - region.optDouble("offset");
            start[axis] -= pending; end[axis] -= pending;
            double low = box.optDouble(axis), high = low + box.optDouble(axis + 2);
            double delta = start[axis] < low || end[axis] - start[axis] > high - low
                ? start[axis] - low : Math.max(0, end[axis] - high);
            double after = Math.max(0, Math.min(region.optDouble("max_offset"), before + delta));
            if (after != before) changes.put(id, after);
            start[axis] = Math.max(low, start[axis] - (after - before));
            end[axis] = Math.min(high, end[axis] - (after - before));
            if (end[axis] <= start[axis]) return null;
            id = parent;
        }
        return id == null ? changes : null;
    }

    boolean canReveal(JSONObject frame, JSONObject target) { return revealOffsets(frame, target) != null; }

    boolean reveal(JSONObject frame, JSONObject target) {
        Map<String, Double> changes = revealOffsets(frame, target);
        if (changes == null || changes.isEmpty()) return false;
        offsets.putAll(changes); revision++;
        return true;
    }

    boolean canScroll(String id, int direction) {
        return canScroll(id, direction, axis(id));
    }

    boolean canScroll(String id, int direction, int requestedAxis) {
        for (int depth = 0; id != null && depth < 129; depth++) {
            JSONObject region = regions.get(id);
            if (region == null) break;
            double value = offset(id);
            if (axis(region) == requestedAxis && (direction < 0 ? value > 0.001 : value < region.optDouble("max_offset") - 0.001)) return true;
            id = parent(region);
        }
        return false;
    }

    boolean scrollBy(String id, double delta) {
        if (!Double.isFinite(delta)) return false;
        int requestedAxis = axis(id);
        boolean changed = false;
        // Depth is bounded by the native schema; the extra bound also rejects malformed cycles.
        for (int depth = 0; id != null && depth < 129 && Math.abs(delta) > 0.001; depth++) {
            JSONObject region = regions.get(id);
            if (region == null) break;
            if (axis(region) != requestedAxis) { id = parent(region); continue; }
            double before = offset(id);
            double after = Math.max(0, Math.min(region.optDouble("max_offset"), before + delta));
            if (after != before) { offsets.put(id, after); changed = true; }
            delta -= after - before;
            id = parent(region);
        }
        if (changed) revision++;
        return changed;
    }
}
