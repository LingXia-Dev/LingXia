package com.lingxia.view;

import android.content.Context;
import android.graphics.Rect;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Responsive host shell container for Android phones, foldables, tri-folds, and tablets.
 *
 * Uses zero device-class branching: switches between `compact` (< 600 dp) and `regular`
 * (>= 600 dp, ±24 dp hysteresis) via {@link GpuFoldAdapter}, expands the left sidebar
 * only when `freeWidth >= 820 dp` (otherwise collapses to the 56 dp icon rail), snaps the
 * docked multi-tab aside slot to vertical hinges when present, and collapses hidden asides
 * to the right-edge `‹ N` ear. Mobile hosts omit host-level sidebar Pins.
 *
 * <p>Not yet mounted by the Android host: pages still present one main at a time.
 */
public final class AdaptiveShellLayout extends FrameLayout {
    public static final float DEFAULT_ASIDE_WIDTH_DP = 360f;

    public interface AsideTabListener {
        void onSelectTab(String surfaceId);
        void onToggleCollapsed(boolean collapsed);
        void onToggleMaximized(boolean maximized);
    }

    private final LinearLayout rowContainer;
    private final FrameLayout sidebarSlot;
    private final FrameLayout mainSlot;
    private final LinearLayout asideColumn;
    private final LinearLayout asideTabBar;
    private final FrameLayout asideContentSlot;
    private final Button collapsedEar;

    private String sizeClass = "compact";
    private int mainCount = 1;
    private final List<String> asideTabs = new ArrayList<>();
    private String activeAsideTab = null;
    private boolean asideCollapsed = false;
    private boolean asideMaximized = false;
    private JSONObject latestHinge = null;
    private AsideTabListener asideTabListener = null;

    public AdaptiveShellLayout(Context context) {
        super(context);
        rowContainer = new LinearLayout(context);
        rowContainer.setOrientation(LinearLayout.HORIZONTAL);
        addView(rowContainer, new LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT));

        sidebarSlot = new FrameLayout(context);
        mainSlot = new FrameLayout(context);

        asideColumn = new LinearLayout(context);
        asideColumn.setOrientation(LinearLayout.VERTICAL);
        asideTabBar = new LinearLayout(context);
        asideTabBar.setOrientation(LinearLayout.HORIZONTAL);
        asideTabBar.setGravity(Gravity.CENTER_VERTICAL);
        asideContentSlot = new FrameLayout(context);
        asideColumn.addView(asideTabBar, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(36)));
        asideColumn.addView(asideContentSlot, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));

        rowContainer.addView(sidebarSlot, new LinearLayout.LayoutParams(0, LayoutParams.MATCH_PARENT));
        rowContainer.addView(mainSlot, new LinearLayout.LayoutParams(0, LayoutParams.MATCH_PARENT, 1f));
        rowContainer.addView(asideColumn, new LinearLayout.LayoutParams(0, LayoutParams.MATCH_PARENT));

        collapsedEar = new Button(context);
        collapsedEar.setVisibility(View.GONE);
        collapsedEar.setOnClickListener(v -> {
            asideCollapsed = false;
            if (asideTabListener != null) asideTabListener.onToggleCollapsed(false);
            reconcileLayout();
        });
        LayoutParams earParams = new LayoutParams(dp(28), dp(56), Gravity.END | Gravity.CENTER_VERTICAL);
        addView(collapsedEar, earParams);
    }

    public FrameLayout getSidebarSlot() { return sidebarSlot; }
    public FrameLayout getMainSlot() { return mainSlot; }
    public FrameLayout getAsideSlot() { return asideContentSlot; }
    public String getSizeClass() { return sizeClass; }
    public void setAsideTabListener(AsideTabListener listener) { this.asideTabListener = listener; }

    /** Apply a serialized `LayoutPresentationPlan` JSON from `lingxia-surface`. */
    public void applyPlanJson(String planJson) {
        if (planJson == null) return;
        try {
            JSONObject plan = new JSONObject(planJson);
            String nextClass = plan.optString("sizeClass", sizeClass);
            if ("compact".equals(nextClass) || "regular".equals(nextClass)) {
                sizeClass = nextClass;
            }
            JSONArray mains = plan.optJSONArray("mains");
            mainCount = mains != null ? mains.length() : 1;

            asideTabs.clear();
            JSONObject unified = plan.optJSONObject("unifiedAsideSlot");
            if (unified != null) {
                JSONArray tabs = unified.optJSONArray("tabs");
                if (tabs != null) {
                    for (int i = 0; i < tabs.length(); i++) {
                        String id = tabs.optString(i, "");
                        if (!id.isEmpty()) asideTabs.add(id);
                    }
                }
                activeAsideTab = unified.isNull("activeTab") ? null : unified.optString("activeTab", null);
                asideCollapsed = unified.optBoolean("collapsed", false);
                asideMaximized = unified.optBoolean("maximized", false);
            } else {
                JSONArray asides = plan.optJSONArray("asides");
                if (asides != null) {
                    for (int i = 0; i < asides.length(); i++) {
                        JSONObject item = asides.optJSONObject(i);
                        if (item != null) {
                            String id = item.optString("id", "");
                            if (!id.isEmpty()) asideTabs.add(id);
                        }
                    }
                }
                activeAsideTab = asideTabs.isEmpty() ? null : asideTabs.get(asideTabs.size() - 1);
            }
            latestHinge = plan.optJSONObject("hinge");
            rebuildAsideTabBar();
            reconcileLayout();
        } catch (Exception ignored) {
        }
    }

    /** Snap the aside to a folding-feature crease (window px). Each GpuView tracks its own fold for `env.hinge`. */
    public void updateFoldingFeature(Rect featureBoundsPx, boolean isHorizontal, boolean isHalfOpened) {
        latestHinge = GpuFoldAdapter.toLocalHingeJson(this, featureBoundsPx, isHorizontal, isHalfOpened);
        reconcileLayout();
    }

    @Override
    protected void onSizeChanged(int w, int h, int oldw, int oldh) {
        super.onSizeChanged(w, h, oldw, oldh);
        if (w <= 0) return;
        float density = getResources().getDisplayMetrics().density;
        float widthDp = w / (density > 0f ? density : 1f);
        sizeClass = GpuFoldAdapter.resolveSizeClass(sizeClass, widthDp);
        reconcileLayout();
    }

    private void rebuildAsideTabBar() {
        asideTabBar.removeAllViews();
        Context ctx = getContext();
        for (final String tabId : asideTabs) {
            Button tabBtn = new Button(ctx);
            tabBtn.setAllCaps(false);
            tabBtn.setText(tabId);
            tabBtn.setSelected(tabId.equals(activeAsideTab));
            tabBtn.setOnClickListener(v -> {
                activeAsideTab = tabId;
                if (asideTabListener != null) asideTabListener.onSelectTab(tabId);
                rebuildAsideTabBar();
            });
            asideTabBar.addView(tabBtn, new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.MATCH_PARENT));
        }
        View spacer = new View(ctx);
        asideTabBar.addView(spacer, new LinearLayout.LayoutParams(0, 1, 1f));

        Button maxBtn = new Button(ctx);
        maxBtn.setText("⤢");
        maxBtn.setContentDescription(asideMaximized ? "Restore Aside" : "Maximize Aside");
        maxBtn.setOnClickListener(v -> {
            asideMaximized = !asideMaximized;
            if (asideTabListener != null) asideTabListener.onToggleMaximized(asideMaximized);
            reconcileLayout();
        });
        asideTabBar.addView(maxBtn, new LinearLayout.LayoutParams(dp(36), ViewGroup.LayoutParams.MATCH_PARENT));

        Button collapseBtn = new Button(ctx);
        collapseBtn.setText("⇥");
        collapseBtn.setContentDescription("Collapse Aside");
        collapseBtn.setOnClickListener(v -> {
            asideCollapsed = true;
            if (asideTabListener != null) asideTabListener.onToggleCollapsed(true);
            reconcileLayout();
        });
        asideTabBar.addView(collapseBtn, new LinearLayout.LayoutParams(dp(36), ViewGroup.LayoutParams.MATCH_PARENT));
    }

    private void reconcileLayout() {
        float density = getResources().getDisplayMetrics().density;
        float scale = density > 0f ? density : 1f;
        float totalDp = getWidth() / scale;
        int asideCount = asideTabs.size();
        boolean isRtl = getLayoutDirection() == View.LAYOUT_DIRECTION_RTL;
        String earLabel = isRtl ? (asideCount + " ›") : ("‹ " + asideCount);

        if ("compact".equals(sizeClass)) {
            setSlotWidth(sidebarSlot, 0);
            setSlotWidth(asideColumn, 0);
            asideColumn.setVisibility(View.GONE);
            collapsedEar.setVisibility(asideCount > 0 ? View.VISIBLE : View.GONE);
            collapsedEar.setText(earLabel);
            return;
        }

        boolean showAside = asideCount > 0 && !asideCollapsed;
        float asideDp = 0f;
        if (showAside) {
            if (asideMaximized) {
                asideDp = Math.max(DEFAULT_ASIDE_WIDTH_DP, totalDp - (mainCount > 1 ? GpuFoldAdapter.SIDEBAR_RAIL_DP : 0f));
            } else if (latestHinge != null && "vertical".equals(latestHinge.optString("orientation"))) {
                if (isRtl) {
                    asideDp = Math.max(240f, (float) latestHinge.optDouble("x", totalDp / 2.0));
                } else {
                    float hingeEnd = (float) (latestHinge.optDouble("x", totalDp / 2.0) + latestHinge.optDouble("width", 0.0));
                    asideDp = Math.max(240f, totalDp - hingeEnd);
                }
            } else {
                asideDp = Math.min(DEFAULT_ASIDE_WIDTH_DP, totalDp * 0.45f);
            }
        }

        float freeWidthDp = Math.max(0f, totalDp - asideDp);
        String switcher = GpuFoldAdapter.resolveSwitcherForm(sizeClass, mainCount, freeWidthDp);
        float sidebarDp = "sidebar".equals(switcher)
                ? GpuFoldAdapter.SIDEBAR_EXPANDED_DP
                : ("rail".equals(switcher) ? GpuFoldAdapter.SIDEBAR_RAIL_DP : 0f);

        setSlotWidth(sidebarSlot, dp(sidebarDp));
        setSlotWidth(asideColumn, dp(asideDp));
        asideColumn.setVisibility(showAside ? View.VISIBLE : View.GONE);
        collapsedEar.setVisibility(asideCount > 0 && asideCollapsed ? View.VISIBLE : View.GONE);
        collapsedEar.setText(earLabel);
    }

    private void setSlotWidth(View slot, int px) {
        LinearLayout.LayoutParams lp = (LinearLayout.LayoutParams) slot.getLayoutParams();
        if (lp.width != px) {
            lp.width = px;
            slot.setLayoutParams(lp);
        }
    }

    private int dp(float value) {
        float density = getResources().getDisplayMetrics().density;
        return Math.round(value * (density > 0f ? density : 1f));
    }
}
