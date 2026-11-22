package com.lingxia.view;

import android.graphics.Rect;
import android.view.View;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Adaptive layout geometry & foldable hinge adapter for Android hosts.
 *
 * Uses zero device-class branching: phones, foldables, tri-folds, and tablets
 * share one 600 dp threshold (`compact` | `regular` with ±24 dp hysteresis)
 * and one physical sidebar width budget (`freeWidth >= 820 dp`).
 */
public final class GpuFoldAdapter {
    public static final float COMPACT_MAX_DP = 600f;
    public static final float HYSTERESIS_DP = 24f;
    public static final float SIDEBAR_EXPANDED_DP = 220f;
    public static final float SIDEBAR_RAIL_DP = 56f;
    public static final float SIDEBAR_EXPAND_MIN_FREE_DP = SIDEBAR_EXPANDED_DP + COMPACT_MAX_DP;

    private GpuFoldAdapter() {}

    /** Resolve unified size class (`"compact"` or `"regular"`) with ±24 dp hysteresis. */
    public static String resolveSizeClass(String previous, float widthDp) {
        String raw = widthDp < COMPACT_MAX_DP ? "compact" : "regular";
        if (previous == null || previous.equals(raw)) {
            return raw;
        }
        return Math.abs(widthDp - COMPACT_MAX_DP) < HYSTERESIS_DP ? previous : raw;
    }

    /**
     * Resolve left sidebar form (`"none"`, `"rail"`, or `"sidebar"`) in `regular`
     * without squeezing the main pane below 600 dp.
     */
    public static String resolveSwitcherForm(String sizeClass, int mainCount, float freeWidthDp) {
        if (!"regular".equals(sizeClass) || mainCount <= 1) {
            return "none";
        }
        return freeWidthDp >= SIDEBAR_EXPAND_MIN_FREE_DP ? "sidebar" : "rail";
    }

    /**
     * Project a physical folding-feature bounds rect (in window px) into
     * view-local logical dp coordinates as the `view.env.hinge` JSON payload.
     * Returns `null` when the crease does not cross the view. A seamless fold
     * reports zero thickness, so this cannot use {@link Rect#isEmpty()} or
     * {@link Rect#setIntersect}, which reject zero-width rects.
     */
    public static JSONObject toLocalHingeJson(
            View targetView,
            Rect featureBoundsPx,
            boolean isHorizontal,
            boolean isHalfOpened
    ) {
        if (targetView == null || featureBoundsPx == null) {
            return null;
        }
        int[] loc = new int[2];
        targetView.getLocationInWindow(loc);
        int viewLeft = loc[0], viewTop = loc[1];
        int viewRight = viewLeft + targetView.getWidth(), viewBottom = viewTop + targetView.getHeight();
        int left = Math.max(viewLeft, featureBoundsPx.left);
        int top = Math.max(viewTop, featureBoundsPx.top);
        int right = Math.min(viewRight, featureBoundsPx.right);
        int bottom = Math.min(viewBottom, featureBoundsPx.bottom);
        // Along the crease it must overlap; across it, it must lie strictly inside the view,
        // or the page would split at its own edge.
        boolean crosses = isHorizontal
                ? right > left && featureBoundsPx.top > viewTop && featureBoundsPx.bottom < viewBottom
                : bottom > top && featureBoundsPx.left > viewLeft && featureBoundsPx.right < viewRight;
        if (!crosses) {
            return null;
        }
        float density = targetView.getResources().getDisplayMetrics().density;
        if (density <= 0f) {
            density = 1f;
        }
        try {
            JSONObject hinge = new JSONObject();
            hinge.put("x", (left - viewLeft) / density);
            hinge.put("y", (top - viewTop) / density);
            hinge.put("width", Math.max(0, right - left) / density);
            hinge.put("height", Math.max(0, bottom - top) / density);
            hinge.put("orientation", isHorizontal ? "horizontal" : "vertical");
            hinge.put("posture", isHalfOpened ? "half-opened" : "flat");
            return hinge;
        } catch (JSONException ignored) {
            return null;
        }
    }
}
