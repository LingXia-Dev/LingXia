package com.lingxia.lxapp

import com.lingxia.app.CurrentLxApp
import com.lingxia.lxapp.chrome.NavigationBarState
import com.lingxia.lxapp.chrome.NavigationBar
import com.lingxia.lxapp.chrome.TabBarState
import com.lingxia.lxapp.chrome.TabBar
import com.lingxia.lxapp.chrome.TabBarShape
import com.lingxia.lxapp.chrome.TabBarInsets
import com.lingxia.lxapp.chrome.TabBarOverflowSheet
import com.lingxia.lxapp.chrome.CapsuleButton
import com.lingxia.lxapp.chrome.CapsuleMenuBottomSheet
import com.lingxia.lxapp.chrome.LxAppTheme
import android.app.Activity
import android.app.UiModeManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.KeyEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import org.json.JSONObject
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import android.content.pm.ActivityInfo
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import android.view.animation.AccelerateDecelerateInterpolator
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import com.lingxia.app.LxLog
import androidx.lifecycle.Lifecycle
import com.lingxia.app.NativeApi
import com.lingxia.app.PageWebViewCallback
import com.lingxia.app.Lingxia
import com.lingxia.app.PermissionManager
import com.lingxia.app.UpdateManager
import com.lingxia.lxapp.APIs.LxAppSurface
import com.lingxia.lxapp.NativeComponents.NativeBridge

/**
 * One lxapp shown in a region of [LxAppActivity]: its page stack, navigation bar, tab bar,
 * and the transitions between its pages. The window itself (insets, orientation, system
 * bars, input) stays with the activity.
 */
internal class LxAppPane(
    val activity: LxAppActivity,
    val root: FrameLayout,
    /** The main region's pane owns the window: its bars, orientation and background. */
    val isMain: Boolean = true,
) {
    companion object {
        private const val TAG = "LingXia.WebView"
        /** GPU navigation stages, at debug level: transitions are timing-sensitive. */
        private const val NAV_TAG = "LingXia.Nav"
        /** Above the navigation and tab bars, whatever their elevation. */
        private const val TOP_LAYER_Z = 10_000f
        /** A first screen retired while awaited is looked up again, this many times. */
        private const val FIRST_SCREEN_RESOLVE_LIMIT = 3
        private const val RETIRED_GPU_CONTAINER = "retired_gpu_container"
        /** Most tabs a floating bar holds: five 60 dp cells clear a fold's crease. */
        private const val FLOATING_TAB_BAR_MAX_ITEMS = 5
    }

    lateinit var appId: String
    var currentSessionId: Long = 0L
    lateinit var webViewContainer: FrameLayout
    var pullToRefreshHelper: PullToRefreshHelper? = null
    var tabBar: TabBar? = null
    /** The tab bar as the lxapp declares it, before [adaptTabBar] fits it to the window. */
    var declaredTabBar: TabBarState? = null
    var navigationBar: NavigationBar? = null

    val hasAppId: Boolean get() = ::appId.isInitialized

    /** The status bar over this pane's top; an aside's region already starts below it. */
    fun statusInset(): Int = if (isMain) LxAppActivity.getStatusBarHeight(activity) else 0

    /** Takes the pane's pages off screen without closing them: the runtime keeps them alive. */
    fun detach() {
        ++gpuTransitionVersion
        clearGpuTransitionCover()
        (currentWebView as? WebView)?.let { NativeBridge.notifyPageInactive(it) }
        currentWebView?.pause()
        currentWebView = null
        if (hasWebViewContainer) webViewContainer.removeAllViews()
        pullToRefreshHelper?.detach()
        (root.parent as? ViewGroup)?.removeView(root)
    }
    val hasWebViewContainer: Boolean get() = ::webViewContainer.isInitialized

    fun destroy() {
        pendingGpuDisposals.toList().forEach { it.run() }
        ++gpuTransitionVersion
        clearGpuTransitionCover()
        pullToRefreshHelper?.detach()
        // Destroy native components before pausing WebView
        currentWebView?.let { (it as? WebView)?.let { NativeBridge.notifyPageDestroyed(it) } }
        // Pause current WebView but don't destroy it: WebView destruction is managed by native.
        currentWebView?.pause()
    }

    // Tracks the currently visible WebView instance
    internal var currentWebView: View? = null
        set(value) {
            field = value
            (value as? GpuPage)?.setChromeListener(gpuChromeListener)
            (value as? GpuPage)?.setTopLayerListener(gpuTopLayerListener)
            (value as? GpuPage)?.setChromeSnapshotListener(gpuChromeSnapshotListener)
            applyGpuChrome((value as? GpuPage)?.chrome)
            applyGpuTopLayer((value as? GpuPage)?.hasTopLayer() == true)
        }

    internal val gpuTopLayerListener = java.util.function.BiConsumer<GpuPage, Boolean> { page, modal ->
        if (page === currentWebView) applyGpuTopLayer(modal)
    }

    /** An open dialog of the GPU page on screen lifts the pages above the bars, which then take no touches. */
    internal fun applyGpuTopLayer(modal: Boolean) {
        if (!hasWebViewContainer) return
        // Under the dialog the bars are out of reach for assistive tech too.
        val hidden = if (modal) View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS else View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
        navigationBar?.importantForAccessibility = hidden
        tabBar?.importantForAccessibility = hidden
        cancelGpuRaise()
        if (modal) {
            topLayerOpen = true
            barSnapshots.clear()
            // The page comes over the bars once a frame draws them under its dialog, so no frame
            // shows neither; a page that never does is raised anyway.
            if (gpuBarsLaidOut()) {
                if (captureGpuBars()) scheduleGpuRaise() else raiseGpuPage(true)
            } else {
                scheduleGpuRaise()
                recaptureGpuBars()
            }
        } else {
            if (barSnapshots.isNotEmpty()) (currentWebView as? GpuPage)?.declineChromeSnapshot()
            barSnapshots.clear()
            topLayerOpen = false
            raiseGpuPage(false)
        }
    }

    internal val gpuChromeSnapshotListener = java.util.function.BiConsumer<GpuPage, Long> { page, generation ->
        if (page === currentWebView) gpuChromeSnapshot(generation)
    }
    internal var topLayerOpen = false
    /** The generation of the bar snapshot handed to the page, by edge. */
    internal val barSnapshots = mutableMapOf<Int, Long>()
    /** Raises the page when no frame shows its bar snapshots in time. */
    internal var gpuRaise: Runnable? = null

    internal fun raiseGpuPage(up: Boolean) {
        webViewContainer.translationZ = if (up) TOP_LAYER_Z else 0f
    }

    internal fun scheduleGpuRaise() {
        cancelGpuRaise()
        val raise = Runnable {
            gpuRaise = null
            if (topLayerOpen) raiseGpuPage(true)
        }
        gpuRaise = raise
        webViewContainer.postDelayed(raise, 250)
    }

    internal fun cancelGpuRaise() {
        gpuRaise?.let { webViewContainer.removeCallbacks(it) }
        gpuRaise = null
    }

    /** A frame on screen draws the bar snapshots up to `generation`; 0 when it draws none. */
    internal fun gpuChromeSnapshot(generation: Long) {
        if (!topLayerOpen || barSnapshots.isEmpty()) return
        if (generation == 0L) {
            // The page dropped them (its surface changed): the real bars show until it has new ones.
            barSnapshots.clear()
            cancelGpuRaise()
            raiseGpuPage(false)
            recaptureGpuBars()
        } else if (generation >= (barSnapshots.values.maxOrNull() ?: Long.MAX_VALUE)) {
            cancelGpuRaise()
            raiseGpuPage(true)
        }
    }

    /** A bar under the dialog changed: hand the page a fresh copy once the change has drawn. */
    internal fun recaptureGpuBars() {
        if (!topLayerOpen || !hasWebViewContainer || recaptureQueued) return
        recaptureQueued = true
        // Before the next draw, after layout: a bar drawn before its layout shows a new title
        // broken to the old title's width.
        val observer = webViewContainer.viewTreeObserver
        observer.addOnPreDrawListener(object : android.view.ViewTreeObserver.OnPreDrawListener {
            override fun onPreDraw(): Boolean {
                if (webViewContainer.viewTreeObserver.isAlive) {
                    webViewContainer.viewTreeObserver.removeOnPreDrawListener(this)
                }
                recaptureQueued = false
                if (topLayerOpen && captureGpuBars() && webViewContainer.translationZ == 0f && gpuRaise == null) {
                    scheduleGpuRaise()
                }
                return true
            }
        })
        webViewContainer.invalidate()
    }
    internal var recaptureQueued = false

    internal fun gpuBarsLaidOut(): Boolean =
        listOfNotNull<View>(navigationBar, tabBar).none { it.isLayoutRequested }

    /** Copies each bar on an edge of the page into it, to draw under its top layer. */
    internal fun captureGpuBars(): Boolean {
        val page = currentWebView as? GpuPage ?: return false
        if (page.width <= 0 || page.height <= 0) return false
        val pageAt = IntArray(2).also { page.getLocationInWindow(it) }
        var handed = false
        for (bar in listOfNotNull<View>(navigationBar, tabBar)) {
            if (!bar.isShown || bar.alpha <= 0.01f || bar.width <= 0 || bar.height <= 0) continue
            val at = IntArray(2).also { bar.getLocationInWindow(it) }
            val x = at[0] - pageAt[0]
            val y = at[1] - pageAt[1]
            val rect = android.graphics.Rect(x, y, x + bar.width, y + bar.height)
            if (!rect.intersect(0, 0, page.width, page.height)) continue
            if (rect.width() < page.width / 2 || rect.height() >= page.height / 2) continue
            val edge = when {
                rect.top <= 0 -> com.lingxia.view.GpuView.EDGE_TOP
                rect.bottom >= page.height -> com.lingxia.view.GpuView.EDGE_BOTTOM
                else -> continue
            }
            val bitmap = android.graphics.Bitmap.createBitmap(bar.width, bar.height, android.graphics.Bitmap.Config.ARGB_8888)
            bar.draw(android.graphics.Canvas(bitmap))
            val generation = page.chromeSnapshot(edge, bitmap, x.toFloat(), y.toFloat())
            bitmap.recycle()
            if (generation > 0) {
                barSnapshots[edge] = generation
                handed = true
            }
        }
        return handed
    }

    /** The host chrome of the GPU page on screen; null on WebView pages, which keep their config. */
    internal var gpuChrome: com.lingxia.view.GpuView.Chrome? = null
    internal val gpuChromeListener = java.util.function.BiConsumer<GpuPage, com.lingxia.view.GpuView.Chrome> { page, chrome ->
        if (page === currentWebView) applyGpuChrome(chrome)
    }

    /** Chrome around a GPU page takes the colours its palette resolves to; only the page on screen paints it. */
    internal fun applyGpuChrome(chrome: com.lingxia.view.GpuView.Chrome?) {
        if (chrome === gpuChrome) return
        gpuChrome = chrome
        tabBar?.setTheme(chrome?.let { TabBar.Theme(it.bar, it.contentMuted, it.accent, it.barLine) })
        navigationBar?.setTheme(chrome?.let { NavigationBar.Theme(it.bar, it.content, it.barLine) })
        // The bars take the page's colours: the snapshots under a dialog follow.
        recaptureGpuBars()
        applyCanvasBackground()
        if (chrome != null) {
            syncNavigationBarToTabBar(tabBar?.visibility == View.VISIBLE)
            val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
            applyStatusBarGlyphs(null)
            if (controller.isAppearanceLightNavigationBars != !chrome.dark) controller.isAppearanceLightNavigationBars = !chrome.dark
        }
    }
    internal var gpuTransitionVersion = 0L
    /** The show a running push slide delivers at its end; null when none is running. */
    internal var slideShow: Runnable? = null
    /** Ends the running slide at once; null when none runs. */
    internal var slideFinish: Runnable? = null

    internal var gpuTransitionCover: android.widget.ImageView? = null
    internal var gpuOutgoingCover: android.widget.ImageView? = null
    /** The outgoing page's copy in flight and the transitions waiting on it, in arrival order. */
    internal var outgoingCapture: Pair<GpuPage, MutableList<Pair<Long, Runnable>>>? = null
    internal val pendingGpuDisposals = mutableSetOf<Runnable>()

    internal var lastReportedShellWidthDp: Int = -1
    internal var lastReportedSurfaceWidthDp: Int = -1
    internal var lastReportedSurfaceHeightDp: Int = -1


    internal fun shouldShowCapsuleButton(targetAppId: String, targetSessionId: Long): Boolean {
        if (!isMain || activity.isMediaFullscreen) return false
        if (targetAppId.isBlank() || targetSessionId <= 0L) return false

        val homeAppId = LxApp.homeAppId ?: return false
        if (targetAppId == homeAppId) return false

        val current = NativeApi.getCurrentLxApp() ?: return false
        if (!current.isValid()) return false

        return current.appId == targetAppId && current.sessionId == targetSessionId
    }

    internal fun reportSurfaceWidthIfNeeded() {
        if (!hasWebViewContainer ||
            !hasAppId || appId.isBlank()
        ) return
        val density = activity.resources.displayMetrics.density
        val window = activity.contentRoot()
        val shellWidthPx = (window.width - window.paddingLeft - window.paddingRight)
            .coerceAtLeast(0)
        val viewportWidthPx = (webViewContainer.width - webViewContainer.paddingLeft - webViewContainer.paddingRight)
            .coerceAtLeast(0)
        // The app's surface, not the current page's area: a page's title bar or tab bar
        // must not change the context every View reads on a tab switch.
        val viewportHeightPx = (root.height - root.paddingTop - root.paddingBottom)
            .coerceAtLeast(0)
        val shellWidthDp = (shellWidthPx / density).toInt()
        val viewportWidthDp = (viewportWidthPx / density).toInt()
        val viewportHeightDp = (viewportHeightPx / density).toInt()
        runCatching {
            if (isMain && shellWidthDp > 0 && shellWidthDp != lastReportedShellWidthDp) {
                lastReportedShellWidthDp = shellWidthDp
                NativeApi.setSurfaceWidth(appId, shellWidthDp.toDouble())
            }
            if (viewportWidthDp > 0 && viewportHeightDp > 0 &&
                (viewportWidthDp != lastReportedSurfaceWidthDp ||
                    viewportHeightDp != lastReportedSurfaceHeightDp)
            ) {
                lastReportedSurfaceWidthDp = viewportWidthDp
                lastReportedSurfaceHeightDp = viewportHeightDp
                NativeApi.setSurfaceViewport(
                    appId,
                    viewportWidthDp.toDouble(),
                    viewportHeightDp.toDouble()
                )
            }
        }.onFailure { error ->
            Log.w(TAG, "Failed to report surface layout: ${error.message}")
        }
    }


    internal fun observeGpuChromeInsets() {
        val pageBounds = android.graphics.Rect()
        val barBounds = android.graphics.Rect()
        root.viewTreeObserver.addOnPreDrawListener {
            val page = currentWebView as? GpuPage
            if (page != null) {
                var left = 0
                var top = 0
                var right = 0
                var bottom = 0
                // A GPU page spans the screen: the navigation bar over its top is chrome too.
                val navigation = navigationBar
                if (page.getGlobalVisibleRect(pageBounds) && navigation != null && navigation.isShown &&
                    navigation.getGlobalVisibleRect(barBounds) && barBounds.intersect(pageBounds) &&
                    barBounds.top <= pageBounds.top) {
                    top = barBounds.bottom - pageBounds.top
                }
                val bar = tabBar
                // The page's own tab bar, not the one on screen: that can still be the outgoing
                // page's on the incoming page's first pre-draw, and a wrong first sample
                // rebuilds its first frame.
                val box = bar?.let { laidOutBox(it) }
                if (bar != null && box != null && !activity.isMediaFullscreen && tabBarOver(page)) {
                    val at = IntArray(2).also { page.getLocationInWindow(it) }
                    val root = IntArray(2).also { root.getLocationInWindow(it) }
                    val x = at[0] - root[0]
                    val y = at[1] - root[1]
                    val overlap = KeptPageGeometry.overlap(
                        KeptPageGeometry.Box(x, y, x + page.width, y + page.height), box,
                        KeptPageGeometry.Chrome(navigationBar = false, tabBar = true, fullscreen = false, gpu = true),
                        bar.config.position,
                    )
                    left = overlap.left
                    right = overlap.right
                    bottom = overlap.bottom
                }
                page.setChromeInsets(left, top, right, bottom)
            }
            reportKeptChromeInsets()
            true
        }
    }

    /**
     * A kept page's insets are those it will be shown with, from its own chrome under the
     * current bars; measured on screen they would be the shown page's, and its last frame
     * would no longer be current when it returns.
     */
    internal fun reportKeptChromeInsets() {
        val bar = tabBar ?: return
        if (!com.lingxia.view.GpuView.keepsSurfaces() || !hasWebViewContainer) return
        val barBox = laidOutBox(bar) ?: return
        val root = IntArray(2).also { root.getLocationInWindow(it) }
        val at = IntArray(2)
        for (index in 0 until webViewContainer.childCount) {
            val child = webViewContainer.getChildAt(index) as? ViewGroup ?: continue
            val page = child.getChildAt(0) as? GpuPage ?: continue
            if (page === currentWebView || child.visibility != View.VISIBLE) continue
            val chrome = ownChrome[child] ?: continue
            page.getLocationInWindow(at)
            val left = at[0] - root[0]
            val top = at[1] - root[1]
            val overlap = KeptPageGeometry.overlap(
                KeptPageGeometry.Box(left, top, left + page.width, top + page.height),
                barBox, chrome, bar.config.position,
            )
            val navigation = if (chrome.gpu && chrome.navigationBar) currentBars().navigationBar else 0
            page.setChromeInsets(overlap.left, navigation, overlap.right, overlap.bottom)
        }
    }

    /** Whether the tab bar shows over [page], by the core's rule for its path. */
    internal fun tabBarOver(page: GpuPage): Boolean {
        val path = normalizePath(page.getCurrentPath())
        if (path.isEmpty()) return tabBar?.visibility == View.VISIBLE
        return NativeApi.tabBarVisibleOn(appId, path)
    }

    /** The bar's laid-out frame in root coordinates: it keeps its place while hidden. */
    internal fun laidOutBox(bar: View): KeptPageGeometry.Box? {
        if (bar.width <= 0 || bar.height <= 0) return null
        val root = IntArray(2).also { root.getLocationInWindow(it) }
        val at = IntArray(2).also { bar.getLocationInWindow(it) }
        return KeptPageGeometry.Box(at[0] - root[0], at[1] - root[1],
            at[0] - root[0] + bar.width, at[1] - root[1] + bar.height)
    }



    internal fun setupTabBar(declared: TabBarState?, preserveGpuFrame: Boolean = true) {
        val page = currentWebView as? GpuPage
        if (preserveGpuFrame && page != null && gpuTransitionCover == null &&
            (declared?.visible == true) != (tabBar?.visibility == View.VISIBLE)) {
            val version = gpuTransitionVersion
            // Runtime chrome events precede navigation. Cover the old geometry
            // before changing it; navigation will own the cover if it follows.
            prepareGpuTransition(version, Runnable {
                // A navigation in the same batch replaced the page and owns the cover and its slide.
                val owned = { version == gpuTransitionVersion && currentWebView === page }
                page.afterNextPresentation { success ->
                    if (owned()) {
                        if (!success) clearGpuTransitionCover()
                        else page.capturePreview { frame ->
                            if (!owned() || activity.isFinishing || activity.destroyed) frame?.recycle()
                            else if (frame == null) clearGpuTransitionCover()
                            else {
                                showGpuTransitionFrame(frame, page)
                                val committed = Runnable { root.postOnAnimation {
                                    if (owned()) clearGpuTransitionCover()
                                } }
                                if (Build.VERSION.SDK_INT >= 29 && root.isHardwareAccelerated) {
                                    root.viewTreeObserver.registerFrameCommitCallback(committed)
                                    root.invalidate()
                                } else root.postOnAnimation(committed)
                            }
                        }
                    }
                }
                setupTabBar(declared, false)
                // A navigation that followed the chrome change has already paused this page.
                if (currentWebView === page) page.resume()
            })
            return
        }
        declaredTabBar = declared
        val shape = declared?.let { touchTabBarShape(it) } ?: TabBarShape.STRIP
        val config = declared?.let { adaptTabBar(it, shape) }
        if (config == null) {
            tabBar?.let { bar ->
                (bar.parent as? ViewGroup)?.removeView(bar)
            }
            tabBar = null
            syncNavigationBarToTabBar(visible = false)
            updateLayoutMargins()
            return
        }

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        // The system navigation bar is deliberately not coloured here: it
        // follows the TabBar that is actually on screen, and showTabBar() is
        // what decides that. Colouring it now would paint a plate for a page
        // whose TabBar is about to be hidden.

        if (tabBar == null) {
            tabBar = TabBar(activity).apply {
                gpuChrome?.let { setTheme(TabBar.Theme(it.bar, it.contentMuted, it.accent, it.barLine)) }
                setConfig(config, shape)
                setOnTabSelectedListener { index, path ->
                    // Use new UI event API
                    dispatchTabClick(index)
                }
                setOnMoreRequestedListener { indices ->
                    val bar = this@apply
                    TabBarOverflowSheet.show(
                        activity,
                        bar,
                        bar.config,
                        indices,
                        onPick = { index ->
                            bar.setSelectedIndex(index, notifyListener = false)
                            dispatchTabClick(index)
                        },
                        onDismiss = { bar.setMoreOpen(false) }
                    )
                }
                applyTabBarLayoutParams(this, config)
            }

            if (isTabBarTransparent) {
                if (webViewContainer.parent == null) {
                    root.addView(webViewContainer)
                }
                root.addView(tabBar)
            } else {
                root.addView(tabBar)
            }
        } else {
            tabBar?.setConfig(config, shape)
            tabBar?.let { tb -> applyTabBarLayoutParams(tb, config) }
            // Config changed; re-apply for whatever is on screen right now.
            syncNavigationBarToTabBar(visible = tabBar?.visibility == View.VISIBLE)
        }

        updateLayoutMargins()
    }

    /**
     * A large screen's tabs (short side regular) leave the phone's strip: a pill floating at the
     * bottom-left while the window is clearly portrait (at least 6:5), else a column at the
     * right edge (Apple's iPhone Duo bar placement) — also for more tabs than the pill holds.
     * A docked aside takes the right edge, and the column moves to the left one.
     */
    internal fun touchTabBarShape(config: TabBarState): TabBarShape {
        val width = root.width
        val height = root.height
        if (!isMain || width <= 0 || height <= 0 || config.position != TabBarState.Position.BOTTOM) return TabBarShape.STRIP
        // A phone keeps its strip even on its side: only a large screen (an open foldable, a
        // tablet; Android's sw600dp) moves the tabs. The smallest width holds through rotation.
        if (activity.resources.configuration.smallestScreenWidthDp < com.lingxia.view.GpuFoldAdapter.COMPACT_MAX_DP) {
            return TabBarShape.STRIP
        }
        return if (height * 5 >= width * 6 && config.list.size <= FLOATING_TAB_BAR_MAX_ITEMS)
            TabBarShape.FLOATING else TabBarShape.COLUMN
    }

    /** The declared bar refitted to [shape]; every item gets a cell, so nothing folds into "more". */
    internal fun adaptTabBar(config: TabBarState, shape: TabBarShape): TabBarState = when (shape) {
        TabBarShape.STRIP -> config
        TabBarShape.FLOATING -> config.copy(dimension = TabBar.FLOATING_STRIP_DP, overflowStartIndex = -1)
        // A column takes its own width, so an immersive bar's see-through background has nothing to show.
        TabBarShape.COLUMN -> config.copy(
            position = columnPosition(),
            dimension = TabBar.COLUMN_WIDTH_DP,
            overflowStartIndex = -1,
            backgroundColor = if (config.isBackgroundTransparent()) gpuChrome?.bar ?: Color.WHITE
                else config.backgroundColor,
        )
    }

    private fun columnPosition() =
        if (activity.asideDocked()) TabBarState.Position.LEFT else TabBarState.Position.RIGHT

    /**
     * Refits the bar when the window crosses a shape boundary (rotation, fold, resize), or a
     * column's edge changes with a docked aside.
     */
    internal fun refitTabBarShape() {
        val declared = declaredTabBar ?: return
        val bar = tabBar ?: return
        val shape = touchTabBarShape(declared)
        if (shape == bar.shape && (shape != TabBarShape.COLUMN || bar.config.position == columnPosition())) return
        setupTabBar(declared, false)
        showTabBar(bar.visibility == View.VISIBLE)
        root.requestApplyInsets()
        if (isMain && activity.hasAsideRegion()) activity.aside.placeEar()
    }

    internal fun tabColumnWidth(): Int {
        val bar = tabBar ?: return 0
        if (bar.shape != TabBarShape.COLUMN || bar.visibility != View.VISIBLE || activity.isMediaFullscreen) return 0
        return bar.layoutParams?.width ?: 0
    }

    /** The column's width on the trailing edge; zero while it stands on the leading one. */
    internal fun trailingColumnWidth(): Int =
        if (tabBar?.config?.position == TabBarState.Position.LEFT) 0 else tabColumnWidth()

    private fun leadingColumnWidth(): Int = tabColumnWidth() - trailingColumnWidth()

    /**
     * Beside a tab column the navigation bar and capsule span the main only, and the status
     * bar is one band in the column's colour: a real status bar has a single tint, and its
     * icons sit over the column.
     */
    internal fun layoutBesideTabColumn() {
        val column = tabColumnWidth()
        navigationBar?.let { nav ->
            (nav.layoutParams as? FrameLayout.LayoutParams)?.let { lp ->
                val left = leadingColumnWidth()
                val right = trailingColumnWidth()
                if (lp.leftMargin != left || lp.rightMargin != right) {
                    lp.leftMargin = left
                    lp.rightMargin = right
                    nav.layoutParams = lp
                }
            }
        }
        layoutCapsuleButton()
        if (!isMain) return
        if (column == 0 && !activity.asideDocked()) {
            val band = activity.statusBand?.takeIf { it.visibility == View.VISIBLE } ?: return
            band.visibility = View.GONE
            applyStatusBarGlyphs(NativeApi.getNavigationBarState(appId, currentWebView?.getCurrentPath() ?: ""))
        } else {
            val band = activity.statusBand ?: View(activity).also {
                activity.statusBand = it
                root.addView(it, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0))
            }
            val height = statusInset()
            (band.layoutParams as FrameLayout.LayoutParams).let { lp ->
                if (lp.height != height) {
                    lp.height = height
                    band.layoutParams = lp
                }
            }
            band.setBackgroundColor(statusBandColor())
            if (band.visibility != View.VISIBLE) {
                band.visibility = View.VISIBLE
                root.bringChildToFront(band)
            }
            applyStatusBarGlyphs(null)
        }
    }

    /** The band's colour: the tab column's beside one, else a neutral one beside a docked aside. */
    fun statusBandColor(): Int = tabBar?.takeIf { tabColumnWidth() > 0 }?.config?.backgroundColor
        ?: gpuChrome?.bar
        ?: (navigationBar?.takeIf { it.visibility == View.VISIBLE }?.background as? android.graphics.drawable.ColorDrawable)?.color
        ?: Color.WHITE

    internal fun applyTabBarLayoutParams(tabBar: TabBar, config: TabBarState) {
        val isVertical = config.position == TabBarState.Position.LEFT || config.position == TabBarState.Position.RIGHT
        // Use configured dimension (Rust provides default value)
        val tabBarSizePx = tabBar.calculatedThicknessPx(config)

        val tabBarBgColor = config.backgroundColor
        val isTabBarTransparent = tabBarBgColor == Color.TRANSPARENT ||
                                 (tabBarBgColor?.let { Color.alpha(it) < 255 } == true)

        (tabBar.layoutParams as? FrameLayout.LayoutParams)?.apply {
            if (isVertical) {
                width = tabBarSizePx
                height = ViewGroup.LayoutParams.MATCH_PARENT
                gravity = when (config.position) {
                    TabBarState.Position.LEFT -> Gravity.START
                    TabBarState.Position.RIGHT -> Gravity.END
                    else -> Gravity.START
                }
                // Add top margin to avoid status bar for vertical TabBars
                topMargin = statusInset()
                bottomMargin = 0
            } else {
                width = ViewGroup.LayoutParams.MATCH_PARENT
                height = tabBarSizePx
                gravity = Gravity.BOTTOM
                applyHorizontalTabBarInsets(tabBar, this, tabBarSizePx, isTabBarTransparent)
            }
            if (tabBar.shape == TabBarShape.FLOATING) applyFloatingTabBarInsets(tabBar, this, tabBarSizePx)
            tabBar.layoutParams = this
        } ?: run {
            // Create new LayoutParams with correct initial dimensions
            val newLayoutParams = if (isVertical) {
                FrameLayout.LayoutParams(tabBarSizePx, ViewGroup.LayoutParams.MATCH_PARENT).apply {
                    gravity = when (config.position) {
                        TabBarState.Position.LEFT -> Gravity.START
                        TabBarState.Position.RIGHT -> Gravity.END
                        else -> Gravity.START
                    }
                    // Add top margin to avoid status bar for vertical TabBars
                    topMargin = statusInset()
                    bottomMargin = 0
                }
            } else {
                FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, tabBarSizePx).apply {
                    gravity = Gravity.BOTTOM
                    applyHorizontalTabBarInsets(tabBar, this, tabBarSizePx, isTabBarTransparent)
                }
            }
            if (tabBar.shape == TabBarShape.FLOATING) applyFloatingTabBarInsets(tabBar, newLayoutParams, tabBarSizePx)
            tabBar.layoutParams = newLayoutParams
        }
    }

    /**
     * A floating bar's strip reaches the screen's bottom edge; the pill sits above the
     * navigation bar or gesture zone. The strip itself is see-through, so the page or the
     * canvas shows around the pill.
     */
    internal fun applyFloatingTabBarInsets(tabBar: TabBar, params: FrameLayout.LayoutParams, tabBarSizePx: Int) {
        params.height = tabBarSizePx + activity.systemBottomInset
        params.bottomMargin = 0
        tabBar.setPadding(0, 0, 0, activity.systemBottomInset)
    }

    internal fun applyHorizontalTabBarInsets(
        tabBar: TabBar,
        params: FrameLayout.LayoutParams,
        tabBarSizePx: Int,
        transparent: Boolean,
    ) {
        val extra = TabBarInsets.extraBottomPx(
            activity.systemBottomInset,
            activity.navBarHasBottomInset,
            activity.resources.displayMetrics.density,
            transparent = transparent,
        )
        if (transparent) {
            // Lift above the navigation bar or gesture zone; the page paints underneath.
            params.height = tabBarSizePx
            params.bottomMargin = extra
            tabBar.setPadding(tabBar.paddingLeft, tabBar.paddingTop, tabBar.paddingRight, 0)
        } else {
            // Grow the opaque strip so its background fills the gesture zone
            // and the icons stay in the original 64dp.
            params.height = tabBarSizePx + extra
            params.bottomMargin = 0
            tabBar.setPadding(tabBar.paddingLeft, tabBar.paddingTop, tabBar.paddingRight, extra)
        }
    }

    internal fun updateLayoutMargins() {
        if (activity.isMediaFullscreen) {
            navigationBar?.visibility = View.GONE
            tabBar?.visibility = View.GONE
            activity.statusBand?.visibility = View.GONE
            FrameLayout.LayoutParams(webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
                topMargin = 0
                bottomMargin = 0
                leftMargin = 0
                rightMargin = 0
                webViewContainer.updateFrameLayoutParamsIfChanged(this)
            }
            val container = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
            container?.translationY = 0f
            return
        }
        val bars = currentBars()
        val chrome = KeptPageGeometry.Chrome(
            navigationBar = navigationBar?.visibility == View.VISIBLE,
            tabBar = tabBar?.visibility == View.VISIBLE,
            fullscreen = activity.isPageFullscreen,
            gpu = currentWebView is GpuPage,
        )
        val margins = KeptPageGeometry.margins(chrome, bars, activity.imeContentBottomInset)
        FrameLayout.LayoutParams(webViewContainer.layoutParams as FrameLayout.LayoutParams).apply {
            topMargin = margins.top
            bottomMargin = margins.bottom
            leftMargin = margins.left
            rightMargin = margins.right
            webViewContainer.updateFrameLayoutParamsIfChanged(this)
        }
        val isTabBarTransparent = bars.tabBarTransparent

        val container = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
        container?.translationY = if (!isTabBarTransparent) calculateWebViewTranslationY() else 0f
        if (container != null) ownChrome[container] = chrome
        layoutBesideTabColumn()
        layoutKeptPages()
        pullToRefreshHelper?.restorePageOffset()
    }

    /** The chrome each page container had while it was the current one. */
    internal val ownChrome = java.util.WeakHashMap<ViewGroup, KeptPageGeometry.Chrome>()

    internal fun currentBars(): KeptPageGeometry.Bars {
        val background = tabBar?.config?.backgroundColor
        val navigation = navigationBar
        return KeptPageGeometry.Bars(
            // The page starts below the navbar content and the status bar.
            navigationBar = if (navigation != null)
                navigation.getCalculatedContentHeightPx() + statusInset() else 0,
            tabBarHeight = tabBar?.layoutParams?.height ?: 0,
            tabBarWidth = tabBar?.layoutParams?.width ?: 0,
            tabBarTransparent = background == Color.TRANSPARENT ||
                (background?.let { Color.alpha(it) < 255 } == true),
            position = tabBar?.config?.position,
            systemBottom = activity.systemBottomInset,
        )
    }

    /** Frames every kept GPU page container under its own chrome; see [KeptPageGeometry]. */
    internal fun layoutKeptPages() {
        if (!com.lingxia.view.GpuView.keepsSurfaces()) return
        val parent = webViewContainer.parent as? View ?: return
        val width = parent.width - parent.paddingLeft - parent.paddingRight
        val height = parent.height - parent.paddingTop - parent.paddingBottom
        if (width <= 0 || height <= 0) return
        val params = webViewContainer.layoutParams as? ViewGroup.MarginLayoutParams ?: return
        val current = KeptPageGeometry.Margins(params.topMargin, params.bottomMargin, params.leftMargin, params.rightMargin)
        val bars = currentBars()
        for (index in 0 until webViewContainer.childCount) {
            val child = webViewContainer.getChildAt(index) as? ViewGroup ?: continue
            if (child.tag == "current_webview_container" || child.getChildAt(0) !is GpuPage) continue
            val own = KeptPageGeometry.margins(ownChrome[child] ?: continue, bars)
            val frame = KeptPageGeometry.frame(width, height, own, current)
            child.translationX = frame.translationX
            child.translationY = frame.translationY
            val size = child.layoutParams
            if (size?.width != frame.width || size.height != frame.height)
                child.layoutParams = FrameLayout.LayoutParams(frame.width, frame.height)
        }
    }



    // Get navbar state
    internal fun getNavBarState(appId: String, path: String): NavigationBarState? {
        return NativeApi.getNavigationBarState(appId, path)
    }

    // Helper function to attach a WebView to the container and resume it
    internal fun attachWebViewToUI(view: View?) {
        if (view == null) {
            LxLog.e(TAG, "attachWebViewToUI called with null view!")
            return
        }
        if (!activity.destroyed) {

            // Ensure view is visible
            view.visibility = View.VISIBLE

            val existingWrapper = (view.parent as? ViewGroup)?.takeIf { it.parent == webViewContainer }

            val container = existingWrapper ?: FrameLayout(activity).apply {
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                tag = "current_webview_container"

                if (view.parent != null && view.parent != this) {
                    (view.parent as? ViewGroup)?.removeView(view)
                }
                view.layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT
                )
                addView(view)
            }

            if (existingWrapper == null) {
                webViewContainer.addView(container)
            } else {
                webViewContainer.bringChildToFront(container)
                container.tag = "current_webview_container"
            }

            // WebViews are created on the application context, so the lxapp's
            // localNightMode override never reaches their theme resolution;
            // feed them the activity configuration once they join the tree so
            // prefers-color-scheme matches the resolved appearance.
            view.dispatchConfigurationChanged(activity.resources.configuration)

            // Attach native bridge for component overlay
            (view as? WebView)?.let { NativeBridge.attachIfNeeded(it) }

            ensurePullToRefreshHelper().attachToWebView(view)
            updatePullToRefreshEnabledForPath(view.getCurrentPath())

            // Resume the WebView's activities
            view.resume()
        } else {
            Log.w(TAG, "attachWebViewToUI: Activity is destroyed, skipping WebView attachment")
        }
    }

    /** How a container was asked to show a page, kept while its WebView is not ready. */
    internal class Presentation(
        val isReplace: Boolean,
        val isBackNavigation: Boolean,
        val firstScreen: Boolean
    )

    /** Ticket of the latest [present]; an older wait that resolves later is dropped. */
    internal var presentTicket = 0

    /**
     * Show the first page: the app's current one. A navigation issued before
     * this activity existed had nobody to present it, so the page the open
     * named may no longer be the one to show.
     */
    internal fun presentFirstScreen(resolvesLeft: Int = FIRST_SCREEN_RESOLVE_LIMIT) {
        present(null, Presentation(isReplace = true, isBackNavigation = false, firstScreen = true), resolvesLeft)
    }

    /**
     * Show the page the runtime named. [webtag] names one page instance exactly
     * (null: the app's current page). A page's WebView is created
     * asynchronously, so it may not exist yet; the runtime then calls back once
     * it does, and a newer request supersedes the older one.
     */
    internal fun present(webtag: String?, request: Presentation, resolvesLeft: Int = FIRST_SCREEN_RESOLVE_LIMIT) {
        val ticket = ++presentTicket
        val ready = NativeApi.awaitPageView(appId, currentSessionId, webtag) { webView, status ->
            activity.runOnUiThread { onPresentResult(ticket, webtag, request, webView, status, resolvesLeft) }
        }
        if (ready != null) showPresentation(ready, request)
    }

    internal fun onPresentResult(
        ticket: Int,
        webtag: String?,
        request: Presentation,
        webView: View?,
        status: Int,
        resolvesLeft: Int
    ) {
        if (ticket != presentTicket || activity.isFinishing || activity.destroyed) {
            LxLog.d(NAV_TAG, "present $webtag superseded (ticket $ticket, now $presentTicket)")
            return
        }
        if (webView != null) {
            showPresentation(webView, request)
            return
        }
        when {
            // The awaited page was retired; the first screen follows the app's
            // current page, a navigation has its own newer request.
            status == PageWebViewCallback.GONE && request.firstScreen && resolvesLeft > 0 -> {
                presentFirstScreen(resolvesLeft - 1)
                return
            }
            status == PageWebViewCallback.GONE && !request.firstScreen -> {
                Log.d(TAG, "Page ${webtag ?: "current"} is gone; a newer navigation supersedes it")
                return
            }
        }
        clearGpuTransitionCover()
        // The switch did not happen: the page frozen for it stays the current one.
        (currentWebView as? GpuPage)?.resume()
        LxLog.e(TAG, "WebView not available for appId=$appId, page=${webtag ?: "current"} (status=$status)")
        if (currentWebView == null) finishWithSessionClose("initial_webview_missing")
    }

    internal fun showPresentation(webView: View, request: Presentation) {
        if (request.firstScreen) {
            setupWebViewContentWithExisting(webView)
            // Resumed while it had no page: the resume-time notification found
            // nobody to tell.
            if (activity.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                (webView as? WebView)?.let { NativeBridge.notifyPageActive(it) }
            }
        } else {
            navigateToPage(webView, request.isReplace, request.isBackNavigation)
        }
    }

    internal fun finishWithSessionClose(reason: String) {
        if (hasAppId && appId.isNotBlank() && currentSessionId > 0L) {
            val closed = runCatching { notifyLxAppClosed(currentSessionId) }.getOrElse { error ->
                Log.w(TAG, "finishWithSessionClose notify failed (reason=$reason): ${error.message}")
                false
            }
            if (!closed) {
                Log.w(TAG, "finishWithSessionClose stale/ignored close (reason=$reason, appId=$appId, sessionId=$currentSessionId)")
            }
        }
        // Prevent onStop() from dispatching onAppHide for a session that is already closed.
        if (!isMain) {
            activity.dropAside(appId)
            return
        }
        appId = ""
        currentSessionId = 0L
        activity.finish()
    }

    // New method to setup WebView content with an existing WebView
    internal fun setupWebViewContentWithExisting(webView: View) {
        // Set the current WebView first
        this.currentWebView = webView

        // Attach and resume immediately
        attachWebViewToUI(webView)

        // Update navbar and statusbar for initial page
        val currentPath = webView.getCurrentPath() ?: ""
        val navbarState = NativeApi.getNavigationBarState(appId, currentPath)
        if (navbarState != null) {
            updateNavigationBar(navbarState, false, true, currentPath)
        }
        updatePullToRefreshEnabledForPath(currentPath)

        // Apply page orientation
        if (isMain) activity.applyPageOrientation(currentPath)

        // Trigger onPageShow for initial WebView (this is the single place for initial page show)
        val shown = webView.pageIdentity()
        if (webView.getAppId() != null && shown != null) {
            NativeApi.onPageShow(webView.getAppId()!!, shown)
        }
    }

    // Function to setup the FrameLayout that holds the WebViews
    internal fun setupWebViewContainer() {
        webViewContainer = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            // Use transparent background to respect page configuration
            setBackgroundColor(Color.TRANSPARENT)
            addOnLayoutChangeListener { _, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom ->
                if (right - left != oldRight - oldLeft || bottom - top != oldBottom - oldTop) {
                    reportSurfaceWidthIfNeeded()
                }
            }
        }

        if (webViewContainer.parent == null) {
            root.addView(webViewContainer)
        }

        ensurePullToRefreshHelper()
    }

    /**
     * The part of the revealed strip the status bar covers: its height when no
     * navigation bar sits above the page (`navigationStyle: custom`), nothing
     * otherwise.
     */
    internal fun pullToRefreshTopInset(): Int {
        if (activity.isMediaFullscreen || activity.isPageFullscreen) return 0
        if (navigationBar?.visibility == View.VISIBLE) return 0
        return statusInset()
    }

    internal fun ensurePullToRefreshHelper(): PullToRefreshHelper {
        if (pullToRefreshHelper == null) {
            pullToRefreshHelper = PullToRefreshHelper(
                activity,
                webViewContainer,
                topInset = ::pullToRefreshTopInset,
                spinnerColor = ::resolveRefreshSpinnerColor,
            ) { handlePullToRefresh() }
        }
        return pullToRefreshHelper!!
    }

    /**
     * The host theme's refresh spinner colour for the current scheme:
     * `accentColor`, else `mutedForegroundColor`. Undeclared, it falls back
     * to LingXia's own chrome palette, not the activity theme: a host that
     * never chose an accent would otherwise get the AppCompat default.
     */
    internal fun resolveRefreshSpinnerColor(): Int {
        val dark = (activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        return runCatching { NativeApi.refreshIndicatorColor(dark) }
            .getOrNull()
            ?.takeIf { it.isNotEmpty() }
            ?.let { hex -> runCatching { Color.parseColor(hex) }.getOrNull() }
            ?: com.lingxia.lxapp.chrome.OverlayPalette.of(activity).body
    }

    internal fun normalizePath(rawPath: String?): String {
        if (rawPath.isNullOrEmpty()) return ""
        return rawPath.substringBefore('?').substringBefore('#')
    }

    internal fun updatePullToRefreshEnabledForPath(path: String?) {
        val helper = pullToRefreshHelper ?: return

        // A page whose config cannot be read has not opted in.
        val normalized = normalizePath(path)
        if (normalized.isEmpty()) {
            helper.setEnabled(false)
            return
        }

        val enabled = try {
            NativeApi.isPullDownRefreshEnabled(appId, normalized)
        } catch (e: Exception) {
            LxLog.e(TAG, "Failed to read pull-to-refresh config for $path: ${e.message}")
            false
        }
        helper.setEnabled(enabled)
    }

    internal fun handlePullToRefresh() {
        val helper = pullToRefreshHelper ?: return
        if (!helper.isEnabled()) {
            helper.endRefreshing()
            return
        }

        val webtag = currentWebView?.getWebTag()
        if (webtag.isNullOrEmpty()) {
            helper.endRefreshing()
            return
        }

        // The runtime is told which page instance was pulled, by its webtag.
        try {
            NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_PULL_DOWN_REFRESH, webtag)
        } catch (e: Exception) {
            LxLog.e(TAG, "onLxappEvent pull-to-refresh failed: ${e.message}")
            helper.endRefreshing()
        }
    }

    internal fun addCapsuleButton(navbarState: NavigationBarState? = null) {
        if (!shouldShowCapsuleButton(appId, currentSessionId)) return

        val density = activity.resources.displayMetrics.density
        val statusBarHeight = statusInset()
        val capsuleHeightPx = (LxAppTheme.Metrics.CAPSULE_HEIGHT_DP * density).toInt()
        val capsuleTopMarginPx = LxAppTheme.Metrics.calculateCapsuleTopMargin(statusBarHeight, density)

        val state = navbarState ?: NativeApi.getNavigationBarState(
            appId,
            currentWebView?.getCurrentPath() ?: ""
        )
        val capsule = CapsuleButton(
            activity,
            state?.capsuleBackgroundColor ?: NavigationBarState.DEFAULT_CAPSULE_BACKGROUND_COLOR,
            state?.capsuleForegroundColor ?: NavigationBarState.DEFAULT_CAPSULE_FOREGROUND_COLOR,
            state?.capsuleDividerColor ?: NavigationBarState.DEFAULT_CAPSULE_DIVIDER_COLOR,
            state?.capsuleInteractionColor ?: NavigationBarState.DEFAULT_CAPSULE_INTERACTION_COLOR
        ).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                capsuleHeightPx
            ).apply {
                gravity = Gravity.TOP or Gravity.END
                topMargin = capsuleTopMarginPx
                rightMargin = (LxAppTheme.Metrics.CAPSULE_TRAILING_MARGIN_DP * density).toInt() + trailingColumnWidth()
            }
            setOnMenuClickListener {
                CapsuleMenuBottomSheet.show(activity, appId)
            }
            setOnCloseClickListener {
                NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_CAPSULE_CLICK, NativeApi.CAPSULE_ACTION_CLOSE)
            }
        }

        root.post {
            root.removeView(root.findViewWithTag("capsule_button"))
            root.addView(capsule)
        }
    }

    internal fun layoutCapsuleButton() {
        val capsule = root.findViewWithTag<View>("capsule_button") ?: return
        val lp = capsule.layoutParams as? FrameLayout.LayoutParams ?: return
        val density = activity.resources.displayMetrics.density
        val top = LxAppTheme.Metrics.calculateCapsuleTopMargin(statusInset(), density)
        val trailing = (LxAppTheme.Metrics.CAPSULE_TRAILING_MARGIN_DP * density).toInt() + trailingColumnWidth()
        if (lp.topMargin != top || lp.rightMargin != trailing) {
            lp.topMargin = top
            lp.rightMargin = trailing
            capsule.layoutParams = lp
            capsule.post { NativeApi.syncLxAppHostUI(appId) }
        }
    }

    fun getCapsuleRectJSON(requestingAppId: String): String {
        if (requestingAppId != appId) {
            return "null"
        }

        val capsuleView = root.findViewWithTag<View>("capsule_button") ?: return "null"
        if (!capsuleView.isShown) {
            return "null"
        }

        val widthPx = capsuleView.width
        val heightPx = capsuleView.height
        if (widthPx <= 0 || heightPx <= 0) {
            return "null"
        }

        val page = currentWebView ?: return "null"
        if (page.width <= 0 || page.height <= 0) {
            return "null"
        }

        val density = activity.resources.displayMetrics.density
        val cap = IntArray(2)
        val origin = IntArray(2)
        capsuleView.getLocationInWindow(cap)
        page.getLocationInWindow(origin)
        val left = (cap[0] - origin[0]) / density.toDouble()
        val top = (cap[1] - origin[1]) / density.toDouble()
        val width = widthPx / density.toDouble()
        val height = heightPx / density.toDouble()

        Log.i(
            TAG,
            "Capsule rect: left=${String.format("%.1f", left)} top=${String.format("%.1f", top)} " +
                "width=${String.format("%.1f", width)} height=${String.format("%.1f", height)}"
        )

        return JSONObject().apply {
            put("width", width)
            put("height", height)
            put("top", top)
            put("right", left + width)
            put("bottom", top + height)
            put("left", left)
        }.toString()
    }


    internal fun notifyLxAppClosed(sessionId: Long = currentSessionId): Boolean {
        return NativeApi.onLxAppClosed(appId, sessionId)
    }


    /** [webtag] names the page instance to show; null: the app's current page. */
    internal fun navigate(webtag: String?, animationType: AnimationType): Boolean {
        if (!hasAppId) return false
        val version = ++gpuTransitionVersion
        LxLog.d(NAV_TAG, "navigate $webtag $animationType v$version cover=${gpuTransitionCover != null}")
        ++presentTicket
        if (animationType == AnimationType.BACKWARD) activity.beginBackHold(version)
        else activity.releaseBackHold()
        if (gpuOutgoingCover != null) clearGpuTransitionCover()
        // A replace between kept surfaces needs no cover; see navigateToPage.
        val covered = !(animationType == AnimationType.NONE && com.lingxia.view.GpuView.keepsSurfaces())
        val navigate = Runnable {
            if (version != gpuTransitionVersion || activity.isFinishing || activity.destroyed) return@Runnable
            // Frozen before the chrome changes: re-laid out under the next page's bars, the
            // outgoing page would repaint shifted and its last frame would no longer be its own.
            val outgoing = (currentWebView as? GpuPage)?.takeIf { !covered }
            outgoing?.freeze()
            try {
                syncTabBarFromRuntime()
                present(webtag, Presentation(
                    isReplace = animationType == AnimationType.NONE,
                    isBackNavigation = animationType == AnimationType.BACKWARD,
                    firstScreen = false
                ))
            } catch (error: Exception) {
                clearGpuTransitionCover()
                if (currentWebView === outgoing) outgoing?.resume()
                LxLog.e(TAG, "Navigation failed: ${error.message}", error)
            }
        }
        if (currentWebView is GpuPage && gpuTransitionCover == null && covered) {
            prepareGpuTransition(version, navigate)
        } else {
            navigate.run()
        }
        return true
    }

    internal fun dispatchTabClick(index: Int) {
        val target = tabBar?.config?.list?.getOrNull(index)?.pagePath
        val previous = currentWebView
        if (target == previous?.getCurrentPath()) {
            NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_TABBAR_CLICK, index.toString())
            return
        }
        val version = ++gpuTransitionVersion
        LxLog.d(NAV_TAG, "tab click $index v$version")
        val dispatch = Runnable {
            if (version != gpuTransitionVersion || activity.isFinishing || activity.destroyed) return@Runnable
            val handled = NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_TABBAR_CLICK, index.toString())
            if (handled == 0 && version == gpuTransitionVersion) clearGpuTransitionCover()
        }
        if (previous is GpuPage && target != previous.getCurrentPath() && gpuTransitionCover == null &&
            !com.lingxia.view.GpuView.keepsSurfaces())
            prepareGpuTransition(version, dispatch)
        else dispatch.run()
    }

    internal fun prepareGpuTransition(version: Long, navigate: Runnable) {
        val previous = currentWebView as? GpuPage ?: return navigate.run()
        // A chrome change and the navigation after it cover the same frame: one copy serves both.
        outgoingCapture?.takeIf { it.first === previous }?.let {
            it.second.add(version to navigate)
            return
        }
        val waiters = mutableListOf(version to navigate)
        outgoingCapture = previous to waiters
        previous.captureReadyPreview { preview ->
            if (outgoingCapture?.second === waiters) outgoingCapture = null
            val current = waiters.filter { it.first == gpuTransitionVersion }
            LxLog.d(NAV_TAG, "outgoing preview ${previous.getWebTag()} ready=${preview != null} for ${waiters.map { "v${it.first}" }} now v$gpuTransitionVersion")
            if (current.isEmpty() || activity.isFinishing || activity.destroyed) {
                preview?.recycle()
                return@captureReadyPreview
            }
            if (preview != null) {
                showGpuTransitionFrame(preview, previous)
            }
            val committed = Runnable {
                webViewContainer.postOnAnimation { current.forEach { it.second.run() } }
            }
            if (Build.VERSION.SDK_INT >= 29 && webViewContainer.isHardwareAccelerated) {
                webViewContainer.viewTreeObserver.registerFrameCommitCallback(committed)
                webViewContainer.invalidate()
            } else webViewContainer.postOnAnimation(committed)
        }
    }

    fun preserveGpuPageBeforeDisposal(page: GpuPage, release: Runnable) {
        if (currentWebView !== page || !page.isShown || activity.isFinishing || activity.destroyed || gpuTransitionCover != null) {
            release.run()
            return
        }
        val releaseOnce = object : Runnable {
            override fun run() {
                if (pendingGpuDisposals.remove(this)) release.run()
            }
        }
        pendingGpuDisposals.add(releaseOnce)
        // Pop/replace removes the runtime page before the host navigation event.
        // Detach its registry identity immediately, but retain pixels until the
        // replacement window buffer covers its SurfaceView.
        page.capturePreview { frame ->
            if (frame != null && currentWebView === page && !activity.isFinishing && !activity.destroyed && gpuTransitionCover == null)
                showGpuTransitionFrame(frame, page)
            else frame?.recycle()
            if (gpuTransitionCover != null && Build.VERSION.SDK_INT >= 29 && root.isHardwareAccelerated) {
                root.viewTreeObserver.registerFrameCommitCallback { root.post(releaseOnce) }
                root.invalidate()
            } else root.postOnAnimation(releaseOnce)
        }
    }

    internal fun clearGpuTransitionCover() {
        slideFinish = null
        // Cancelling the slide skips its end action; the show it carried is still owed.
        slideShow?.let { show ->
            slideShow = null
            LxLog.d(NAV_TAG, "slide cut short; showing now")
            show.run()
        }
        listOfNotNull(gpuTransitionCover, gpuOutgoingCover).forEach {
            it.animate().cancel()
            (it.parent as? ViewGroup)?.removeView(it)
            it.setImageDrawable(null)
        }
        gpuTransitionCover = null
        gpuOutgoingCover = null
    }

    internal fun animateGpuTransition(outgoing: android.graphics.drawable.Drawable,
                                     backwards: Boolean, version: Long, page: View) {
        val incoming = gpuTransitionCover ?: return
        val previous = android.widget.ImageView(activity).apply {
            scaleType = android.widget.ImageView.ScaleType.MATRIX
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            setBackgroundColor(android.graphics.Color.BLACK)
            setImageDrawable(outgoing)
            layoutParams = FrameLayout.LayoutParams(incoming.layoutParams as FrameLayout.LayoutParams)
        }
        gpuOutgoingCover = previous
        root.addView(previous, root.indexOfChild(incoming))
        val distance = root.width.toFloat() * if (backwards) -1f else 1f
        incoming.translationX = distance
        val interpolator = android.view.animation.DecelerateInterpolator()
        // Equal progress keeps the opaque edges touching throughout the slide.
        previous.animate().translationX(-distance).setDuration(300L).setInterpolator(interpolator).start()
        val show = Runnable { (page.parent as? FrameLayout)?.let { triggerOnPageShow(it) } }
        slideShow = show
        val gpu = page as? GpuPage
        gpu?.transition(if (backwards) com.lingxia.view.GpuView.TRANSITION_POP else com.lingxia.view.GpuView.TRANSITION_PUSH, 0f)
        var finished = false
        lateinit var finish: Runnable
        finish = Runnable {
            if (finished) return@Runnable
            finished = true
            if (slideFinish === finish) slideFinish = null
            incoming.setOnTouchListener(null)
            // The view's animator keeps listeners for its next animation.
            incoming.animate().setUpdateListener(null)
            gpu?.transition(com.lingxia.view.GpuView.TRANSITION_END, 0f)
            LxLog.d(NAV_TAG, "slide ${page.getWebTag()} done v$version now v$gpuTransitionVersion")
            if (slideShow === show) slideShow = null
            if (version == gpuTransitionVersion) {
                clearGpuTransitionCover()
                show.run()
            }
        }
        // The live page already sits under the slide: a touch falls through to it, and the
        // slide jumps to its end so what is shown is what was touched.
        incoming.isClickable = false
        incoming.setOnTouchListener { _, event ->
            if (event.actionMasked == android.view.MotionEvent.ACTION_DOWN) {
                LxLog.d(NAV_TAG, "touch during slide ${page.getWebTag()}; ending it")
                incoming.post(finish)
            }
            false
        }
        slideFinish = finish
        incoming.animate().translationX(0f).setDuration(300L).setInterpolator(interpolator)
            .setUpdateListener { gpu?.transition(com.lingxia.view.GpuView.TRANSITION_FRAME, incoming.translationX) }
            .withEndAction(finish).start()
    }

    /** [passThrough]: the frame is of the live page under the cover, so touches go to it. */
    internal fun showGpuTransitionFrame(frame: android.graphics.Bitmap, source: View, passThrough: Boolean = false) {
        val origin = IntArray(2)
        val rootOrigin = IntArray(2)
        source.getLocationInWindow(origin)
        root.getLocationInWindow(rootOrigin)
        val snapshot = android.graphics.Bitmap.createBitmap(
            root.width, root.height, android.graphics.Bitmap.Config.ARGB_8888
        )
        val canvas = android.graphics.Canvas(snapshot)
        val previousCover = gpuTransitionCover
        val previousAlpha = previousCover?.alpha
        previousCover?.alpha = 0f
        try {
            root.draw(canvas)
        } finally {
            if (previousAlpha != null) previousCover.alpha = previousAlpha
        }
        canvas.drawBitmap(frame, (origin[0] - rootOrigin[0]).toFloat(),
            (origin[1] - rootOrigin[1]).toFloat(), null)
        // This PixelCopy buffer was only read by the synchronous software canvas;
        // it never enters a display list. Do not retain it until the next GC.
        frame.recycle()
        snapshot.setHasAlpha(false)
        val cover = gpuTransitionCover ?: android.widget.ImageView(activity).apply {
            setBackgroundColor(android.graphics.Color.BLACK)
            scaleType = android.widget.ImageView.ScaleType.MATRIX
            importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
            isClickable = true
            setOnTouchListener { view, event ->
                if (view.isClickable && event.actionMasked == android.view.MotionEvent.ACTION_DOWN)
                    LxLog.i(NAV_TAG, "touch at ${event.x.toInt()},${event.y.toInt()} held by the cover: the incoming page is not shown yet")
                false
            }
            root.addView(this)
            gpuTransitionCover = this
        }
        // Preserve the outgoing chrome too: hiding the navbar exposes an area
        // outside the GPU buffer. Keep the snapshot at its original pixel size.
        cover.layoutParams = FrameLayout.LayoutParams(snapshot.width, snapshot.height).apply {
            leftMargin = -root.paddingLeft
            topMargin = -root.paddingTop
        }
        cover.setImageBitmap(snapshot)
        cover.isClickable = !passThrough
        bringGpuTransitionCoverToFront()
    }

    internal fun bringGpuTransitionCoverToFront() {
        navigationBar?.let { root.bringChildToFront(it) }
        // The snapshot owns navbar pixels until incoming content is presented.
        // Keep tabs interactive so a newer selection can supersede this transition.
        gpuTransitionCover?.let { root.bringChildToFront(it) }
        tabBar?.let { root.bringChildToFront(it) }
        activity.statusBand?.let { root.bringChildToFront(it) }
        root.findViewWithTag<View>("capsule_button")?.let { root.bringChildToFront(it) }
    }

    internal fun syncTabBarFromRuntime() {
        // Reflect visibility from Rust TabBarState only
        val tabBarConfig = NativeApi.getTabBarState(appId)
        val visible = tabBarConfig?.visible ?: false
        showTabBar(visible)
        tabBarConfig?.let {
            tabBar?.setSelectedIndex(it.selectedIndex, notifyListener = false)
        }
    }

    /** Set while the launch cover holds the system bars on our behalf. */
    internal var navBarSyncDeferred = false

    internal fun showTabBar(show: Boolean) {
        if (activity.isMediaFullscreen) {
            activity.pendingTabBarVisibility = if (show) View.VISIBLE else View.GONE
            tabBar?.visibility = View.GONE
            syncNavigationBarToTabBar(visible = false)
            return
        }
        tabBar?.visibility = if (show) View.VISIBLE else View.GONE
        syncNavigationBarToTabBar(visible = show)
        updateLayoutMargins()
    }

    /**
     * Colour the system navigation bar for the TabBar *currently on screen*.
     *
     * The bar sits directly under the TabBar, so matching it is right while the
     * TabBar is showing, and wrong the rest of the time: a page that hides the
     * TabBar owns the bottom of the window, and a bar left in the TabBar's
     * colour paints over it. It also bit any lxapp declaring a `tabBar` with no
     * `backgroundColor` — the colour fell back to white, so every page outside
     * the tabs ended in a white plate.
     */
    internal fun syncNavigationBarToTabBar(visible: Boolean) {
        if (!isMain) return
        // While the launch cover is up it owns the whole window, bars included.
        // The home page's TabBar state lands well before the cover lifts, so
        // colouring the bar now paints the TabBar's colour along the bottom of
        // a full-bleed cover. Hold it transparent and re-apply on dismissal —
        // once, however many times the TabBar state churns under the cover.
        if (!navBarSyncDeferred) {
            navBarSyncDeferred = SplashOverlay.doOnCoverGone {
                navBarSyncDeferred = false
                syncNavigationBarToTabBar(tabBar?.visibility == View.VISIBLE)
            }
        }
        if (navBarSyncDeferred) {
            LxAppActivity.updateNavigationBarTransparency(activity, isTabBarTransparent = true)
            return
        }
        val config = tabBar?.config
        if (!visible || config == null) {
            LxAppActivity.updateNavigationBarTransparency(activity, isTabBarTransparent = true)
            return
        }
        val background = config.backgroundColor
        val isTransparent = background == Color.TRANSPARENT ||
                            (background?.let { Color.alpha(it) < 255 } == true)
        if (tabBar?.shape == TabBarShape.FLOATING) {
            LxAppActivity.updateNavigationBarTransparency(activity, isTabBarTransparent = true)
            return
        }
        val actual: Int = when {
            config.position == TabBarState.Position.BOTTOM || tabBar?.shape == TabBarShape.COLUMN -> background
            else -> 0xFFF8F8F8.toInt() // VERTICAL_TABBAR_BACKGROUND_COLOR
        }
        LxAppActivity.updateNavigationBarTransparency(activity, isTransparent, actual)
    }


    /**
     * Show WebView with appropriate animation and trigger onPageShow
     */
    /**
     * Animate old container out with cleanup
     */
    internal fun animateOldContainerOut(
        oldContainer: ViewGroup,
        oldWebView: View,
        endX: Float,
        duration: Long,
        interpolator: AccelerateDecelerateInterpolator
    ) {
        oldContainer.animate()
            .translationX(endX)
            .setDuration(duration)
            .setInterpolator(interpolator)
            .withEndAction {
                try {
                    // Pause and clean up old WebView
                    oldWebView.pause()
                    oldWebView.visibility = View.GONE

                    // Remove old container from parent
                    (oldContainer.parent as? ViewGroup)?.removeView(oldContainer)

                } catch (e: Exception) {
                    LxLog.e(TAG, "Error cleaning up old container: ${e.message}")
                }
            }
            .start()
    }

    /**
     * Perform WebView transition animation with synchronized navbar animation
     *
     * Extracted from navigateToPage for reuse in coordinated navigation
     */
    internal fun performWebViewTransition(oldWebView: View?, newContainer: FrameLayout, isBackNavigation: Boolean, shouldAnimate: Boolean = true, navbarState: NavigationBarState? = null, deferPageShow: Boolean = false, retireAfter: GpuPage? = null) {
        // Get reference to old container BEFORE adding new one. A reused
        // container is already a child and can still carry the current tag, so
        // exclude it here or the page would be mistaken for its own predecessor.
        val oldContainer = webViewContainer.findViewWithTag<ViewGroup>("current_webview_container")
            ?.takeIf { it !== newContainer }
        oldContainer?.tag = "previous_webview_container" // Re-tag old container
        newContainer.tag = "current_webview_container"
        // The incoming page fills the parent again; kept ones are framed by layoutKeptPages.
        if (com.lingxia.view.GpuView.keepsSurfaces() && newContainer.layoutParams?.width != FrameLayout.LayoutParams.MATCH_PARENT) {
            newContainer.layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT)
            newContainer.translationX = 0f
            newContainer.translationY = 0f
        }

        try {
            if (newContainer.parent == null) {
                webViewContainer.addView(newContainer)
            } else {
                // Retired earlier but never detached, so its surface is intact:
                // showing it is the whole switch.
                newContainer.visibility = View.VISIBLE
                webViewContainer.bringChildToFront(newContainer)
            }
        } catch (e: Exception) {
            LxLog.e(TAG, "Error adding new container to webViewContainer: ${e.message}")
            return
        }
        pruneRetiredDuplicates(newContainer)
        if (gpuTransitionCover != null) bringGpuTransitionCoverToFront()
        // App-context webviews miss the activity's night override; sync the
        // freshly attached tree (see setupWebViewContentWithExisting).
        newContainer.dispatchConfigurationChanged(activity.resources.configuration)

        if (shouldAnimate) {
            // Set up animation based on navigation direction
            val slideInTranslation = if (isBackNavigation) -webViewContainer.width.toFloat() else webViewContainer.width.toFloat()
            val slideOutTranslation = if (isBackNavigation) webViewContainer.width.toFloat() else -webViewContainer.width.toFloat()

            // Set initial position for new container
            newContainer.translationX = slideInTranslation

            // Animate the transition
            val animationDuration = 300L

            // Animate navbar and webview together
            if (navbarState != null && navigationBar != null) {
                animateNavBar(navbarState, isBackNavigation)
            }

            // Update layout margins after navbar state change
            updateLayoutMargins()

            // Animate new container sliding in - SAME TIME AS NAVBAR
            newContainer.animate()
                .translationX(0f)
                .setDuration(animationDuration)
                .setInterpolator(android.view.animation.DecelerateInterpolator())
                .withEndAction {
                    // Trigger onPageShow after animation completes
                    triggerOnPageShow(newContainer)
                }
                .start()

            // Animate old container sliding out (if exists)
            oldContainer?.let { container ->
                container.animate()
                    .translationX(slideOutTranslation)
                    .setDuration(animationDuration)
                    .setInterpolator(android.view.animation.AccelerateInterpolator())
                    .withEndAction {
                        retireContainer(container)
                    }
                    .start()
            }
        } else {
            // No animation - update navbar immediately
            if (navbarState != null && navigationBar != null) {
                updateNavBar(navbarState)
            }

            // Update layout margins after navbar state change
            updateLayoutMargins()

            newContainer.translationX = 0f

            // Take the old container off screen — retired, not necessarily
            // removed; see retireContainer. Under an unpresented GPU page it
            // stays up until that page's first frame, then both swap at once.
            oldContainer?.let { container ->
                if (retireAfter == null) retireContainer(container)
                else retireAfter.revealOnFirstFrame {
                    if (container.tag != "current_webview_container") retireContainer(container)
                }
            }

            // Trigger onPageShow immediately
            if (!deferPageShow) triggerOnPageShow(newContainer)
        }
    }

    /**
     * Trigger onPageShow for WebView container
     *
     * IMPORTANT: Post to next frame to ensure all UI components (including pullToRefreshHelper)
     * are fully initialized before JS onShow is triggered
     */
    internal fun triggerOnPageShow(container: FrameLayout) {
        container.post {
            try {
                val webView = container.getChildAt(0) as? View
                if (webView == null) LxLog.d(NAV_TAG, "onPageShow skipped: empty container")
                if (webView?.getAppId() != null && webView.getCurrentPath() != null) {
                    val pagePath = webView.getCurrentPath()!!
                    LxLog.d(NAV_TAG, "onPageShow ${webView.getWebTag()}")
                    NativeApi.onPageShow(webView.getAppId()!!, webView.pageIdentity() ?: pagePath)
                    if (isMain) activity.applyPageOrientation(pagePath)
                }
            } catch (e: Exception) {
                LxLog.e(TAG, "Failed to call nativeOnPageShow in performWebViewTransition: ${e.message}")
            }
        }
    }

    /**
     * Clean up old container after animation
     */
    /**
     * Paths the TabBar owns. The runtime keeps a tab page alive as a warm
     * singleton, so its view stays in the window between visits rather than
     * being torn down and rebuilt on every switch.
     */
    internal fun isTabPath(path: String?): Boolean {
        if (path.isNullOrEmpty()) return false
        val normalized = path.substringBefore('?').substringBefore('#')
        return tabBar?.config?.list?.any {
            it.pagePath.substringBefore('?').substringBefore('#') == normalized
        } == true
    }

    /**
     * Take [container] off screen.
     *
     * A tab page stays attached and hidden: detaching its WebView from the
     * window destroys the compositor surface, and the blank fill while the
     * surface is rebuilt is exactly what a tab switch would show. Hidden costs
     * no drawing, and the count is bounded by the tab list.
     *
     * Everything else is removed as before — a pushed page is not coming back.
     */
    internal fun retireContainer(container: ViewGroup) {
        val gpu = container.getChildAt(0) as? GpuPage
        gpu?.pause()
        val path = (container.getChildAt(0) as? View)?.getCurrentPath()
        if (gpu != null && isTabPath(path) && com.lingxia.view.GpuView.keepsSurfaces()) {
            // Stays attached behind the current page: the page fades its layer and the
            // runtime decides whether its surface is kept (surface_retention).
            container.translationX = 0f
            container.tag = RETIRED_GPU_CONTAINER
        } else if (isTabPath(path)) {
            container.translationX = 0f
            container.visibility = View.GONE
        } else {
            cleanupOldContainer(container)
        }
    }

    /**
     * Drop a retired container left over from an earlier instance of the page
     * now on screen — a tab page rebuilt by `reLaunch`/`redirectTo` gets a new
     * WebView, and the old one must not linger behind it. Only hidden
     * containers qualify: a visible one is still mid-transition.
     */
    internal fun pruneRetiredDuplicates(keep: ViewGroup) {
        val keepPath = (keep.getChildAt(0) as? View)?.getCurrentPath() ?: return
        val normalized = keepPath.substringBefore('?').substringBefore('#')
        for (index in webViewContainer.childCount - 1 downTo 0) {
            val child = webViewContainer.getChildAt(index) as? ViewGroup ?: continue
            if (child === keep || (child.visibility != View.GONE && child.tag != RETIRED_GPU_CONTAINER)) continue
            val childPath = (child.getChildAt(0) as? View)?.getCurrentPath() ?: continue
            if (childPath.substringBefore('?').substringBefore('#') == normalized) {
                cleanupOldContainer(child)
            }
        }
    }

    internal fun cleanupOldContainer(container: ViewGroup) {
        try {
            webViewContainer.removeView(container)
        } catch (e: Exception) {
            LxLog.e(TAG, "Error cleaning up old container: ${e.message}")
        }
    }

    /**
     * Navigate to a page with coordinated navbar and webview updates
     *
     * IMPROVEMENT: Coordinates navbar and webview updates in the same step
     * @param isReplace Whether this is a replace navigation
     * @param isBackNavigation Whether this is a back navigation
     */
    internal fun navigateToPage(
        newWebView: View,
        isReplace: Boolean = false,
        isBackNavigation: Boolean = false
    ) {
        val version = gpuTransitionVersion
        val animateGpu = newWebView is GpuPage && gpuTransitionCover != null && !isReplace
        fun failedGpuPresentation() {
            LxLog.d(NAV_TAG, "reveal ${newWebView.getWebTag()} failed v$version parent=${newWebView.parent != null}")
            clearGpuTransitionCover()
            activity.releaseBackHold(version)
            if (animateGpu) (newWebView.parent as? FrameLayout)?.let { triggerOnPageShow(it) }
        }
        if (newWebView is GpuPage && gpuTransitionCover != null) {
            var copyRetries = 0
            fun revealPresentedFrame() {
                fun reveal(frame: android.graphics.Bitmap?) {
                    if (version != gpuTransitionVersion || activity.isFinishing || activity.destroyed) {
                        LxLog.d(NAV_TAG, "reveal ${newWebView.getWebTag()} stale v$version now v$gpuTransitionVersion")
                        frame?.recycle()
                        return
                    }
                    if (frame == null) {
                        LxLog.d(NAV_TAG, "reveal ${newWebView.getWebTag()} copy missed v$version retry=$copyRetries")
                        if (++copyRetries > 1) { failedGpuPresentation(); return }
                        newWebView.resume()
                        // A page with nothing new to draw presents nothing by itself; waiting for
                        // its next frame stalled the push. This asks for one, then copies it.
                        newWebView.captureReadyPreview { retried -> reveal(retried) }
                        return
                    }
                    // PixelCopy waits for the submitted GPU buffer, unlike the
                    // presentation callback. Commit those pixels before uncovering.
                    val outgoing = gpuTransitionCover?.drawable
                    if (gpuTransitionCover != null) showGpuTransitionFrame(frame, newWebView, passThrough = !animateGpu)
                    if (animateGpu && outgoing != null) {
                        animateGpuTransition(outgoing, isBackNavigation, version, newWebView)
                        activity.releaseBackHold(version)
                        return
                    }
                    activity.releaseBackHold(version)
                    // The show waited for a slide; another cover owner (a tab bar change) took
                    // the outgoing cover, so no slide runs and the show is due now.
                    if (animateGpu) LxLog.d(NAV_TAG, "reveal ${newWebView.getWebTag()} without slide v$version")
                    val committed = Runnable {
                        webViewContainer.postOnAnimation {
                            if (version == gpuTransitionVersion) clearGpuTransitionCover()
                            if (animateGpu && version == gpuTransitionVersion)
                                (newWebView.parent as? FrameLayout)?.let { triggerOnPageShow(it) }
                        }
                    }
                    if (Build.VERSION.SDK_INT >= 29 && webViewContainer.isHardwareAccelerated) {
                        webViewContainer.viewTreeObserver.registerFrameCommitCallback(committed)
                        webViewContainer.invalidate()
                    } else webViewContainer.postOnAnimation(committed)
                }
                newWebView.capturePreview { frame -> reveal(frame) }
            }
            // A re-pushed page's surface still holds its previous instance's frame; the slide
            // copies the surface, so it waits for the core to reveal a frame of this instance.
            newWebView.afterCurrentFrame { success ->
                LxLog.d(NAV_TAG, "presented ${newWebView.getWebTag()} ok=$success v$version now v$gpuTransitionVersion")
                if (version == gpuTransitionVersion) {
                    if (success) revealPresentedFrame() else failedGpuPresentation()
                }
            }
        } else {
            clearGpuTransitionCover()
            activity.releaseBackHold(version)
        }
        try {
            val targetPath = normalizePath(newWebView.getCurrentPath())
            // Get current WebView before changes
            val oldWebView = currentWebView

            if (oldWebView != null && oldWebView != newWebView) {
                (oldWebView as? WebView)?.let { NativeBridge.notifyPageInactive(it) }
            }

            val navbarState = getNavBarState(appId, targetPath)

            // Keep the container this WebView already lives in when it is
            // still attached. Taking a WebView out of the window destroys its
            // compositor surface, and rebuilding one costs frames the switch
            // cannot hide: the page lands on a blank fill until it composites
            // again — measured at ~8 frames on a mid-range device, which is the
            // flash a tab switch shows. `attachWebViewToUI` already reuses an
            // attached wrapper for the same reason.
            val liveContainer = (newWebView.parent as? FrameLayout)
                ?.takeIf { it.parent === webViewContainer }

            if (liveContainer == null && newWebView.parent != null) {
                (newWebView.parent as? ViewGroup)?.removeView(newWebView)
            }

            // Kept surfaces switch GPU pages without a cover: the incoming page stays off screen
            // over the outgoing one until, after layout, it holds a current frame or presents one.
            val keepSwitch = com.lingxia.view.GpuView.keepsSurfaces() && isReplace &&
                newWebView is GpuPage && oldWebView is GpuPage && oldWebView !== newWebView &&
                gpuTransitionCover == null

            // IMPORTANT: Make sure the new WebView is fully prepared before animation
            newWebView.visibility = View.VISIBLE
            newWebView.resume()

            // Reuse the attached container, or build one for a page arriving
            // fresh. The current-page tag is stamped by the transition, once it
            // has told the incoming container apart from the outgoing one.
            val newContainer = liveContainer ?: FrameLayout(activity).apply {
                layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )

                try {
                    addView(newWebView)
                } catch (e: Exception) {
                    LxLog.e(TAG, "Error adding WebView to container: ${e.message}")
                    return@apply
                }
            }

            (newWebView as? WebView)?.let { NativeBridge.attachIfNeeded(it) }

            ensurePullToRefreshHelper().attachToWebView(newWebView)
            updatePullToRefreshEnabledForPath(targetPath)

            if (oldWebView != newWebView) {
                (newWebView as? WebView)?.let { NativeBridge.notifyPageActive(it) }
            }

            // Use coordinated WebView transition (handles all animation and onPageShow)
            // Only animate for forward/backward navigation, not for replace operations (tab switch, launch, replace)
            val shouldAnimate = !isReplace && !animateGpu
            performWebViewTransition(oldWebView, newContainer, isBackNavigation, shouldAnimate, navbarState, animateGpu,
                retireAfter = if (keepSwitch) newWebView as GpuPage else null)

            // Update the current WebView reference
            currentWebView = newWebView

        } catch (e: Exception) {
            LxLog.e(TAG, "Error in coordinated navigation: ${e.message}", e)
        }
    }

    /** Keep status bar glyphs legible against native chrome or custom content. */
    internal fun applyStatusBarGlyphs(navbarState: NavigationBarState?) {
        if (!isMain) return
        val band = activity.statusBand?.takeIf { it.visibility == View.VISIBLE }
        val bandColor = (band?.background as? android.graphics.drawable.ColorDrawable)?.color
        val lightGlyphs = bandColor?.let { NavigationBar.ColorUtils.isColorDark(it) } ?: gpuChrome?.dark ?: if (navbarState?.showNavbar == true) {
            !NavigationBar.ColorUtils.isColorDark(navbarState.navigationBarForegroundColor)
        } else {
            (activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
                Configuration.UI_MODE_NIGHT_YES
        }
        // Only on a change: an appearance write marks the window attributes changed, and
        // the next traversal pays a window relayout for it even when the value is the same.
        val controller = WindowCompat.getInsetsController(activity.window, activity.window.decorView)
        if (controller.isAppearanceLightStatusBars != !lightGlyphs) {
            controller.isAppearanceLightStatusBars = !lightGlyphs
        }
    }

    internal fun animateNavBar(navbarState: NavigationBarState, isBackNavigation: Boolean) {
        applyStatusBarGlyphs(navbarState)
        if (!navbarState.showNavbar) {
            navigationBar?.visibility = View.GONE
            if (activity.isMediaFullscreen) {
                activity.pendingNavBarVisibility = View.GONE
            }
            return
        }

        navigationBar?.apply {
            visibility = View.VISIBLE
            translationX = if (isBackNavigation) -width.toFloat() else width.toFloat()

            configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = false
            )
        }
        if (activity.isMediaFullscreen) {
            activity.pendingNavBarVisibility = View.VISIBLE
            navigationBar?.visibility = View.GONE
        }
    }

    internal fun updateNavBar(navbarState: NavigationBarState) {
        applyStatusBarGlyphs(navbarState)
        if (!navbarState.showNavbar) {
            navigationBar?.visibility = View.GONE
            if (activity.isMediaFullscreen) {
                activity.pendingNavBarVisibility = View.GONE
            }
            return
        }

        navigationBar?.apply {
            visibility = View.VISIBLE
            configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = true
            )
        }
        if (activity.isMediaFullscreen) {
            activity.pendingNavBarVisibility = View.VISIBLE
            navigationBar?.visibility = View.GONE
        }
    }

    /**
     * Navigation transitions pre-apply the target page's top offset, and the
     * webview/navbar layers animate independently; whatever strip of this
     * container they expose must carry the resolved scheme, not white.
     */
    internal fun applyCanvasBackground() {
        val themed = resolveCanvasColor() ?: return
        if (!isMain) {
            root.setBackgroundColor(themed)
            return
        }
        // While the launch cover is up, the canvas behind it belongs to the
        // launch, not to the app theme. It is not idle scenery either: a home
        // page that redirects on boot tears its WebView down and builds
        // another, and for those frames the canvas is all there is. Painted
        // the theme's colorBackground that reads as a white flash punched
        // through the cover; painted the launch background it is the colour
        // the launch started from.
        val launch = SplashOverlay.backgroundColor(activity)
        val deferred = launch != null && SplashOverlay.doAfterCoverRemoved {
            root.setBackgroundColor(themed)
            activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(themed))
        }
        if (deferred) {
            root.setBackgroundColor(launch!!)
            // The window background too: the frame the cover is detached on is
            // one the WebView may not have composited into yet, and the window
            // is what shows through then — the theme's is a light flash
            // punched through the end of the launch.
            activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(launch))
        } else {
            root.setBackgroundColor(themed)
            // The window as well: a deferred pass may have painted it the
            // launch colour, and an appearance change repaints through here —
            // the two surfaces must not part ways, or the stale one shows
            // through wherever the bars and containers move during navigation.
            activity.window.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(themed))
        }
    }

    /**
     * The colour the canvas behind the page should be.
     *
     * The host's declared page floor first: it is the colour the lxapp paints
     * its own page with, so every strip of canvas the page does not cover —
     * the one a pull-to-refresh opens above it, the one a navigation
     * transition slides views across — reads as part of the page rather than
     * as a seam. The activity theme is the fallback, and a poor one on its
     * own: the generated host manifest asks for a light-only AppCompat theme,
     * so `colorBackground` stays white even in dark mode.
     *
     * Null only when neither source resolves, which leaves the canvas alone
     * rather than painting it a colour we do not believe in.
     */
    internal fun resolveCanvasColor(): Int? {
        gpuChrome?.let { return it.canvas }
        val dark = (activity.resources.configuration.uiMode and
            Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        runCatching { NativeApi.pageBackgroundColor(dark) }
            .getOrNull()
            ?.takeIf { it.isNotEmpty() }
            ?.let { hex -> runCatching { Color.parseColor(hex) }.getOrNull() }
            ?.let { return it }

        val value = android.util.TypedValue()
        if (!activity.theme.resolveAttribute(android.R.attr.colorBackground, value, true)) return null
        return if (value.resourceId != 0) {
            androidx.core.content.ContextCompat.getColor(activity, value.resourceId)
        } else {
            value.data
        }
    }

    internal fun createNavBar() {
        if (navigationBar != null) return

        try {
            val statusBarHeight = statusInset()
            navigationBar = NavigationBar(activity).apply {
                gpuChrome?.let { setTheme(NavigationBar.Theme(it.bar, it.content, it.barLine)) }
                val navBarContentHeight = getCalculatedContentHeightPx()
                layoutParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    navBarContentHeight + statusBarHeight  // Include status bar height
                ).apply {
                    gravity = Gravity.TOP
                    topMargin = 0  // Start from very top
                }

                setPadding(paddingLeft, statusBarHeight, paddingRight, paddingBottom)  // Add status bar padding
                setOnBackButtonClickListener { handleBackButtonClick() }
                setOnHomeButtonClickListener { handleHomeButtonClick() }
                // A page drawing this bar under its dialog needs the new title, not the old one.
                onTitleChanged = Runnable { if (topLayerOpen) recaptureGpuBars() }
                visibility = View.GONE
            }

            root.addView(navigationBar)
        } catch (e: Exception) {
            navigationBar = null
        }
    }

    internal fun updateNavigationBar(config: NavigationBarState?, isBackNavigation: Boolean, disableAnimation: Boolean = false, targetPath: String? = null) {
        val pathForNavbar = targetPath ?: currentWebView?.getCurrentPath() ?: ""
        val navbarState = config ?: NativeApi.getNavigationBarState(appId, pathForNavbar)

        applyStatusBarGlyphs(navbarState)
        if (navbarState != null) {
            // Create navbar if needed
            if (navigationBar == null) {
                createNavBar()
            }

            navigationBar?.configure(
                navbarState = navbarState,
                onBackClickListener = { handleBackButtonClick() },
                onHomeClickListener = { handleHomeButtonClick() },
                disableAnimation = disableAnimation
            )
            if (activity.isMediaFullscreen) {
                val shouldShow = navbarState.showNavbar ||
                    navbarState.showBackButton ||
                    navbarState.showHomeButton
                activity.pendingNavBarVisibility = if (shouldShow) View.VISIBLE else View.GONE
                navigationBar?.visibility = View.GONE
            }
        } else {
            // Hide navbar completely
            navigationBar?.visibility = View.GONE
            if (activity.isMediaFullscreen) {
                activity.pendingNavBarVisibility = View.GONE
            }
        }

        updateLayoutMargins()
        updateCapsuleButton(navbarState)
    }

    /**
     * Handles the click event from the NavigationBar's back button.
     */
    internal fun handleBackButtonClick() {
        NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_NAVIGATION_CLICK, NativeApi.NAVIGATION_ACTION_BACK)
    }

    /**
     * Handles the click event from the NavigationBar's home button.
     */
    internal fun handleHomeButtonClick() {
        NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_NAVIGATION_CLICK, NativeApi.NAVIGATION_ACTION_HOME)
    }


    // Helper to calculate the Y translation based on visible bars
    internal fun calculateWebViewTranslationY(): Float {
        // Since topMargin in updateLayoutMargins() already handles NavigationBar positioning,
        // and we no longer support TOP positioned TabBars, no additional offset is needed
        val tabBarOffset = 0
        // Return only TabBar offset, NavigationBar is handled by topMargin
        return tabBarOffset.toFloat()
    }

    // Close the current LxApp
    fun closeLxApp(sessionId: Long = currentSessionId) {
        // Notify native layer first
        if (!notifyLxAppClosed(sessionId)) {
            Log.w(TAG, "Ignoring stale close callback for appId=$appId sessionId=$sessionId")
            return
        }
        ++gpuTransitionVersion
        clearGpuTransitionCover()

        // Pause and clean up current WebView
        currentWebView?.let { webView ->
            webView.pause()
            webView.visibility = View.GONE
        }
        webViewContainer.removeAllViews()
        currentWebView = null

        // Hide tab bar with animation (capsule and navbar remain)
        showTabBar(false)

        // Clear current app state
        appId = ""

        // Get next LxApp from Rust stack and open it
        val currentLxApp = NativeApi.getCurrentLxApp()
        if (currentLxApp != null && currentLxApp.isValid()) {
            // Switching within this Activity never calls onStart. Re-enter the
            // retained route through the session-checked open lifecycle so the
            // caller receives App.onShow without another App.onLaunch.
            val path = NativeApi.onLxAppOpened(
                currentLxApp.appId, currentLxApp.path, currentLxApp.sessionId
            )
            if (path.isNotBlank()) openLxApp(currentLxApp.appId, currentLxApp.sessionId)
        }
    }

    /** Switch to a different LxApp in the current activity, on its current page. */
    fun openLxApp(appId: String, sessionId: Long) {
        if (sessionId <= 0L) {
            LxLog.e(TAG, "Refusing to open app without valid sessionId: appId=$appId")
            return
        }

        // Ensure all UI operations are on the main thread
        activity.runOnUiThread {
            ++gpuTransitionVersion
            clearGpuTransitionCover()
            // Update app state (no intent extras needed - we're not switching activities)
            this.appId = appId
            this.currentSessionId = sessionId

            // 1. Necessary preparation (build tabbar, etc.)
            prepareLxApp(appId)

            // 2. Check whether to show capsule button (home=hide, others=show)
            updateCapsuleButtonVisibility(appId)

            // 3. Call navigate as entry point
            navigate(null, AnimationType.NONE)
        }
    }

    /**
     *  Necessary preparation for LxApp (build tabbar, etc.)
     */
    internal fun prepareLxApp(appId: String) {
        // Pause current WebView
        currentWebView?.let { webView ->
            webView.pause()
            webView.visibility = View.GONE
        }

        // Clear WebView container for new app
        webViewContainer.removeAllViews()
        currentWebView = null
        pullToRefreshHelper?.setEnabled(false)

        // Build tab bar configuration for new app (tabbar is dynamic).
        //
        // The same setup the first lxapp gets, not just a new config: this app
        // may be the first to declare a strip at all, and the strip's height
        // and the page's bottom margin are both derived here. Setting only the
        // config left the second lxapp laid out for the first one's strip until
        // some later window-insets pass corrected it — and adding the "more"
        // panel to the decor view is exactly such a pass, so the page jumped
        // under the panel the moment it opened.
        val tabBarConfig = NativeApi.getTabBarState(appId)
        setupTabBar(tabBarConfig)
        // Reflect visibility from Rust state, not inferred by presence
        showTabBar(tabBarConfig?.visible == true)
        // Re-measure with the visibility this app actually asked for: the
        // margins above were computed while the previous app's strip was still
        // the one on screen.
        updateLayoutMargins()
        root.requestApplyInsets()
    }

    /**
     * Check whether to show capsule button (home=hide, others=show)
     */
    internal fun updateCapsuleButtonVisibility(appId: String) {
        if (!shouldShowCapsuleButton(appId, currentSessionId)) {
            val capsuleButton = root.findViewWithTag<View>("capsule_button")
            capsuleButton?.visibility = View.GONE
        } else {
            updateCapsuleButton()
        }
    }

    // Update capsule button visibility
    internal fun updateCapsuleButton(navbarState: NavigationBarState? = null) {
        root.post {
            val capsule = root.findViewWithTag<CapsuleButton>("capsule_button")
            if (!shouldShowCapsuleButton(appId, currentSessionId)) {
                capsule?.visibility = View.GONE
            } else {
                if (capsule == null) {
                    addCapsuleButton(navbarState)
                } else {
                    val state = navbarState ?: NativeApi.getNavigationBarState(
                        appId,
                        currentWebView?.getCurrentPath() ?: ""
                    )
                    capsule.applyStyle(
                        state?.capsuleBackgroundColor ?: NavigationBarState.DEFAULT_CAPSULE_BACKGROUND_COLOR,
                        state?.capsuleForegroundColor ?: NavigationBarState.DEFAULT_CAPSULE_FOREGROUND_COLOR,
                        state?.capsuleDividerColor ?: NavigationBarState.DEFAULT_CAPSULE_DIVIDER_COLOR,
                        state?.capsuleInteractionColor ?: NavigationBarState.DEFAULT_CAPSULE_INTERACTION_COLOR
                    )
                    capsule.visibility = View.VISIBLE
                    layoutCapsuleButton()
                }
            }
        }
    }

    // Get current app ID

    // Get current WebView (internal access for LxApp)
    internal fun getCurrentPageWebTag(): String? = currentWebView?.getWebTag()

    internal fun getCurrentWebView(): com.lingxia.lxapp.WebView? = currentWebView as? WebView
}
