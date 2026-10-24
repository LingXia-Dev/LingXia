package com.lingxia.ui;

import android.content.res.Resources;
import android.util.TypedValue;
import org.json.JSONArray;
import org.json.JSONException;

/** Samples the platform's SP conversion, including nonlinear accessibility scaling. */
final class GpuTextScale {
    private GpuTextScale() { }

    static JSONArray measure(Resources resources) throws JSONException {
        android.util.DisplayMetrics metrics = resources.getDisplayMetrics();
        JSONArray points = new JSONArray();
        // Unit samples preserve Android's piecewise curve through ordinary text sizes.
        // Large display sizes use progressively wider intervals to bound the snapshot.
        for (int size = 0; size <= 256; size++) append(points, size, metrics);
        for (int size = 512; size <= 16384; size *= 2) append(points, size, metrics);
        return points;
    }

    private static void append(JSONArray points, int size, android.util.DisplayMetrics metrics)
            throws JSONException {
        float logical = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, size, metrics) / metrics.density;
        points.put(new JSONArray().put(size).put(logical));
    }
}
