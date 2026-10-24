package com.lingxia.ui;

import org.json.JSONArray;
import org.json.JSONObject;

/** Rebind an unchanged control when another node caused a new presented frame. */
final class GpuClick {
    static int find(JSONObject previous, JSONArray nodes) {
        if (previous == null || GpuIme.editable(previous) || (previous.optLong("on_click") == 0 && !GpuPlot.enabled(previous))
                || previous.optString("identity").isEmpty()) return -1;
        for (int i = 0; nodes != null && i < nodes.length(); i++) {
            JSONObject next = nodes.optJSONObject(i);
            if (next == null || (next.optLong("on_click") == 0 && !GpuPlot.enabled(next))
                    || !previous.optString("identity").equals(next.optString("identity"))) continue;
            try {
                JSONObject before = new JSONObject(previous.toString()), after = new JSONObject(next.toString());
                // Callback handles are regenerated on each commit. Geometry,
                // clipping, label and control state must still match exactly.
                before.remove("on_click"); after.remove("on_click");
                if (GpuPlot.plot(before) != null) GpuPlot.plot(before).remove("on_select");
                if (GpuPlot.plot(after) != null) GpuPlot.plot(after).remove("on_select");
                if (before.toString().equals(after.toString())) return i;
            } catch (org.json.JSONException ignored) { return -1; }
        }
        return -1;
    }
}
