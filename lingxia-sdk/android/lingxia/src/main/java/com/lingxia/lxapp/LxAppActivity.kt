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
import com.lingxia.lxapp.APIs.LxAppActionSheet
import com.lingxia.lxapp.APIs.LxAppModal
import com.lingxia.lxapp.APIs.LxAppSurface
import com.lingxia.lxapp.NativeComponents.NativeBridge

/**
 * Animation type enum for page transitions
 */
internal enum class AnimationType(val value: Int) {
    /**
     * No animation - used for Launch/Replace/SwitchTab semantics
     */
    NONE(0),

    /**
     * Forward animation - push-style animation
     */
    FORWARD(1),

    /**
     * Backward animation - pop-style animation
     */
    BACKWARD(2);

    companion object {
        /**
         * Convert AnimationType to string for logging
         */
        fun toString(type: AnimationType): String {
            return when (type) {
                NONE -> "None"
                FORWARD -> "Forward"
                BACKWARD -> "Backward"
            }
        }

        /**
         * Convert integer to AnimationType
         */
        fun fromInt(value: Int): AnimationType {
            return when (value) {
                1 -> FORWARD
                2 -> BACKWARD
                else -> NONE // 0 or any other value
            }
        }
    }
}

internal sealed class HostFileDialogResult {
    /** `names[i]` is the display name of `paths[i]`, or null when the provider reports none. */
    data class Selected(val paths: List<String>, val names: List<String?>) : HostFileDialogResult()
    object Canceled : HostFileDialogResult()
    object Failed : HostFileDialogResult()
}

/**
 * Simple navigation state tracker
 */
internal data class NavigationState(
    val currentPath: String,
    val previousPath: String? = null,
    val isNavigating: Boolean = false
)

class LxAppActivity : AppCompatActivity() {
    private data class MediaFullscreenState(
        val tabBarVisibility: Int,
        val navigationBarVisibility: Int,
        val navigationBarIndex: Int,
        val navigationBarLayoutParams: ViewGroup.LayoutParams?,
        val overlayLayoutParams: FrameLayout.LayoutParams?,
        val overlayTranslationX: Float,
        val overlayTranslationY: Float,
        val rootPaddingLeft: Int,
        val rootPaddingTop: Int,
        val rootPaddingRight: Int,
        val rootPaddingBottom: Int
    )
    companion object {
        private const val TAG = "LingXia.WebView"
        /** GPU navigation stages, at debug level: transitions are timing-sensitive. */
        private const val NAV_TAG = "LingXia.Nav"
        /** The longest a gesture after a back waits for the page it returns to. */
        private const val BACK_HOLD_MS = 1000L
        const val META_FORCE_IMMERSIVE = "com.lingxia.FORCE_IMMERSIVE"
        const val EXTRA_APP_ID = "appId"
        /** Route of the first page: only the orientation to take before its WebView exists. */
        const val EXTRA_PATH = "path"
        const val EXTRA_SESSION_ID = "sessionId"
        internal const val DEFAULT_NAV_BAR_HEIGHT_DP = 44
        /** How long lifting a forced orientation waits for the window to turn back. */
        private const val ORIENTATION_RETURN_TIMEOUT_MS = 1500L

        /**
         * Update TabBar UI for a specific appId
         * In single-activity architecture, updates the current activity's TabBar
         */
        @JvmStatic
        fun updateTabBarUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.paneOf(appId) != null) {
                val updateTask = {
                    try {
                        activity.paneOf(appId)?.setupTabBar(NativeApi.getTabBarState(appId)) != null
                    } catch (e: Exception) {
                        LxLog.e(TAG, "Failed to refresh TabBar from Rust: ${e.message}", e)
                        false
                    }
                }

                if (Looper.myLooper() == Looper.getMainLooper()) {
                    return updateTask()
                }

                // Off the main thread: fire-and-forget so sync callers
                // (badges, style, per-navigation sync) never block. Callers
                // that need the real result use LxApp.updateTabBarUIAsync.
                activity.runOnUiThread { updateTask() }
                return true
            } else {
                // Normal before this app's activity mounts; it reads the state then.
                Log.d(TAG, "No matching activity found for appId: $appId (current: ${activity?.appId})")
                return false
            }
        }

        /**
         * Update NavigationBar UI for a specific appId
         * Triggers the NavigationBar to refresh its state from Rust
         */
        @JvmStatic
        fun updateNavBarUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.paneOf(appId) != null) {
                activity.runOnUiThread {
                    val pane = activity.paneOf(appId) ?: return@runOnUiThread
                    val currentPath = pane.currentWebView?.getCurrentPath() ?: ""
                    pane.updateNavigationBar(null, false, true, currentPath)
                }
                return true
            }
            return false
        }

        /**
         * Update orientation UI for a specific appId
         * Re-applies the current page orientation from native config.
         */
        @JvmStatic
        fun updateOrientationUI(appId: String): Boolean {
            val activity = LxApp.getCurrentActivity()
            if (activity != null && activity.appId == appId) {
                activity.runOnUiThread {
                    val currentPath = activity.mainPane.currentWebView?.getCurrentPath() ?: ""
                    activity.applyPageOrientation(currentPath)
                }
                return true
            }
            Log.w(TAG, "No matching activity found for appId: $appId (current: ${activity?.appId})")
            return false
        }

        // Helper function to get status bar height
        fun getStatusBarHeight(context: Context): Int {
            // Try WindowInsets API (API 23+)
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
                val activity = context as? Activity
                val insetTop = activity?.window?.decorView?.rootWindowInsets
                    ?.let { WindowInsetsCompat.toWindowInsetsCompat(it) }
                    ?.getInsets(WindowInsetsCompat.Type.statusBars())
                    ?.top ?: 0
                if (insetTop > 0) {
                    return insetTop
                }
            }

            // Fallback: use system resource
            var result = 0
            val resourceId = context.resources.getIdentifier("status_bar_height", "dimen", "android")
            if (resourceId > 0) {
                result = context.resources.getDimensionPixelSize(resourceId)
            }
            // Fallback if resource not found (less likely but safe)
            if (result == 0) {
                result = (24 * context.resources.displayMetrics.density).toInt()
            }
            return result
        }

        /**
         * Configures the system bars (status bar and navigation bar) to be transparent
         * and edge-to-edge. This method enables immersive view experience.
         *
         * @param activity The activity whose system bars should be configured
         * @param lightStatusBarIcons Whether the status bar icons should be light (true)
         *        or dark (false); null follows the system theme
         */
        @JvmStatic
        fun configureTransparentSystemBars(activity: AppCompatActivity, lightStatusBarIcons: Boolean? = null) {
            // Enable Edge-to-Edge using WindowCompat
            WindowCompat.setDecorFitsSystemWindows(activity.window, false)

            activity.window.apply {
                addFlags(android.view.WindowManager.LayoutParams.FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)
                statusBarColor = Color.TRANSPARENT
                // The navigation bar too, despite the name only promising it
                // once. Left at the theme default it is opaque (white under
                // Theme.AppCompat.DayNight.NoActionBar), and that default is
                // what the window wears from the moment it is created until
                // the TabBar state arrives — a white bar across the bottom of
                // the launch cover for the length of the boot. Whoever owns
                // the bottom colours it from here on.
                navigationBarColor = Color.TRANSPARENT
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    isNavigationBarContrastEnforced = false
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    navigationBarDividerColor = Color.TRANSPARENT
                }
            }

            val darkTheme = (activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
                Configuration.UI_MODE_NIGHT_YES
            val lightGlyphs = lightStatusBarIcons ?: darkTheme
            WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
                // isAppearanceLight* == true means "bar over a LIGHT background",
                // i.e. dark glyphs — the inverse of light icons.
                isAppearanceLightStatusBars = !lightGlyphs
                isAppearanceLightNavigationBars = !lightGlyphs
            }
        }

        @JvmStatic
        fun updateNavigationBarTransparency(activity: AppCompatActivity, isTabBarTransparent: Boolean, tabBarBackgroundColor: Int? = null) {
            val window = activity.window
            val color = if (isTabBarTransparent) Color.TRANSPARENT else tabBarBackgroundColor ?: Color.WHITE
            // Light backgrounds keep the system contrast scrim; transparent and dark ones do not.
            val contrast = !isTabBarTransparent &&
                (Color.red(color) * 0.299 + Color.green(color) * 0.587 + Color.blue(color) * 0.114) > 128
            // Unchanged bars skip the window update: every call re-dispatches insets to the
            // whole window, which a tab switch would pay for nothing.
            val noLimits = (window.attributes.flags and android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS) != 0
            val unchanged = !noLimits && window.navigationBarColor == color &&
                (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || window.isNavigationBarContrastEnforced == contrast) &&
                (Build.VERSION.SDK_INT < Build.VERSION_CODES.P || window.navigationBarDividerColor == Color.TRANSPARENT)
            if (unchanged) return
            window.apply {
                // Never set FLAG_LAYOUT_NO_LIMITS. It extends the window past
                // the display on several OEMs (Huawei gesture nav especially);
                // a Gravity.BOTTOM TabBar then sits in the clipped region and
                // only the top of the strip remains visible. Edge-to-edge is
                // already enabled via setDecorFitsSystemWindows(false).
                clearFlags(android.view.WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS)
                navigationBarColor = color
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) isNavigationBarContrastEnforced = contrast
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) navigationBarDividerColor = Color.TRANSPARENT
            }
            ViewCompat.requestApplyInsets(window.decorView)
        }
    }

    private lateinit var rootContainer: FrameLayout
    /** The main region's lxapp; asides get panes of their own. */
    internal lateinit var mainPane: LxAppPane
    /** One neutral status band over the main and a side column; see [LxAppPane.layoutBesideTabColumn]. */
    internal var statusBand: View? = null
    internal var appId: String
        get() = mainPane.appId
        set(value) { mainPane.appId = value }
    private val hasAppId: Boolean get() = ::mainPane.isInitialized && mainPane.hasAppId
    private var currentSessionId: Long
        get() = mainPane.currentSessionId
        set(value) { mainPane.currentSessionId = value }
    internal val pullToRefreshHelper: PullToRefreshHelper? get() = mainPane.pullToRefreshHelper
    internal val destroyed: Boolean get() = isDestroyed
    private var isDestroyed = false
    private var presenterReady = false

    internal val canPresentLxApp: Boolean
        get() = presenterReady && !isFinishing && !isDestroyed
    private var hasEnteredBackground = false

    /** The back navigation whose new gestures are held until the page it returns to shows; 0: none. */
    private var backHold = 0L
    private val heldTouches = ArrayList<android.view.MotionEvent>()
    private var holdingGesture = false
    private var replayingTouches = false
    private var replayQueued = false
    internal var systemBottomInset: Int = 0
    internal var imeContentBottomInset: Int = 0
    /** A visible bottom navigation strip; only opaque TabBars reserve it in root padding. */
    internal var navBarHasBottomInset: Boolean = false
    internal var isMediaFullscreen = false
    internal var isPageFullscreen = false  // For page-level fullscreen (landscape + custom navbar)
    private var forceHostImmersive = false
    private var mediaFullscreenState: MediaFullscreenState? = null
    internal var pendingTabBarVisibility: Int? = null
    internal var pendingNavBarVisibility: Int? = null
    private var shouldRestoreOverlayOrder = false
    private var lastDispatchedDeviceOrientation: String? = null
    private var pendingFileChooserCallback: ValueCallback<Array<Uri>>? = null
    private var pendingHostFileDialogCallback: ((HostFileDialogResult) -> Unit)? = null
    private val fileChooserLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        val callback = pendingFileChooserCallback
        pendingFileChooserCallback = null
        callback?.onReceiveValue(resolveFileChooserResult(result.resultCode, result.data))
    }
    private val hostFileDialogLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        val callback = pendingHostFileDialogCallback
        pendingHostFileDialogCallback = null
        callback?.invoke(resolveHostFileDialogResult(result.resultCode, result.data))
    }

    private fun ensureRuntimeReady(
        targetAppId: String,
        requestedPath: String,
        requestedSessionId: Long
    ): Pair<String, Long>? {
        if (LxApp.homeAppId == null) {
            LxLog.e(TAG, "LxApp runtime is not initialized before LxAppActivity creation")
            return null
        }

        var sessionId = NativeApi.getLxAppSessionId(targetAppId)
        if (sessionId <= 0L) {
            sessionId = requestedSessionId
        } else if (requestedSessionId > 0L && requestedSessionId != sessionId) {
            Log.w(TAG, "Ignoring stale intent sessionId=$requestedSessionId for appId=$targetAppId, using runtime sessionId=$sessionId")
        }

        if (sessionId <= 0L) {
            LxLog.e(TAG, "Missing valid runtime session for appId=$targetAppId")
            return null
        }

        val current = NativeApi.getCurrentLxApp()
        val currentMatches = current != null &&
            current.isValid() &&
            current.appId == targetAppId &&
            current.sessionId == sessionId
        val resolvedPath = if (currentMatches && !current.path.isNullOrBlank()) {
            current.path
        } else {
            requestedPath
        }

        return Pair(resolvedPath, sessionId)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // The runtime paints every surface in the lxapp's resolved scheme, so
        // the window must never be force-darkened. Several OEM builds apply
        // force dark to light windows whenever the *system* is dark; with an
        // lxapp pinned to light that inverts the native canvas and, through
        // it, makes the WebView darken the page algorithmically.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.decorView.isForceDarkAllowed = false
        }

        if (LxApp.homeAppId == null) {
            // Android restores the task's top activity after process death, bypassing
            // the host entry point. Its addon and fresh sessions must be created there.
            Lingxia.restartFromLauncher(this)
            return
        }

        // Take the page's orientation before the first frame. The manifest
        // orientation only covers the bootstrap activity; this one would
        // otherwise be created following the sensor and rotate once the page
        // opens and applies its config — a flip the user sees through the
        // launch cover, and one that leaves any WebView warmed while hidden
        // with a stale visual viewport from the pre-rotation window.
        intent.getStringExtra(EXTRA_APP_ID)?.let { launchAppId ->
            runCatching {
                val path = intent.getStringExtra(EXTRA_PATH).orEmpty()
                updateRequestedOrientation(
                    NativeApi.getPageOrientation(launchAppId, mainPane.normalizePath(path))
                )
            }
        }

        if (SplashOverlay.launchActive()) {
            // Do not animate an empty home activity over the bootstrap
            // cover or theme starting window.
            @Suppress("DEPRECATION")
            overridePendingTransition(0, 0)
        }

        rootContainer = FrameLayout(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        // The main's own frame: a docked aside narrows it, and its bars with it.
        val mainHost = FrameLayout(this)
        rootContainer.addView(mainHost, FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        mainPane = LxAppPane(this, mainHost)

        // Configure transparent system bars for edge-to-edge experience
        configureTransparentSystemBars(this)
        forceHostImmersive = isHostImmersiveEnabled()

        // Initialize appId from intent FIRST (check for null)
        appId = intent.getStringExtra(EXTRA_APP_ID) ?: run {
            LxLog.e(TAG, "Missing required parameter: appId")
            finish()
            return
        }
        var initialPath = intent.getStringExtra(EXTRA_PATH) ?: ""
        val requestedSessionId = intent.getLongExtra(EXTRA_SESSION_ID, 0L)
        val resolvedEntry = ensureRuntimeReady(appId, initialPath, requestedSessionId) ?: run {
            finish()
            return
        }
        initialPath = resolvedEntry.first
        currentSessionId = resolvedEntry.second
        if (currentSessionId <= 0L) {
            LxLog.e(TAG, "Runtime returned invalid session for appId=$appId")
            finish()
            return
        }

        // Apply initial page orientation before first frame to avoid visible startup rotation.
        // Full orientation/immersive sync still happens later in setupWebViewContentWithExisting().
        runCatching {
            val initialOrientation = NativeApi.getPageOrientation(appId, mainPane.normalizePath(initialPath))
            updateRequestedOrientation(initialOrientation)
        }.onFailure { error ->
            Log.w(TAG, "Failed to apply initial orientation before first frame: ${error.message}")
        }

        setContentView(rootContainer)
        mainPane.observeGpuChromeInsets()
        rootContainer.addOnLayoutChangeListener { _, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom ->
            if (right - left != oldRight - oldLeft || bottom - top != oldBottom - oldTop) {
                rootContainer.post {
                    if (::aside.isInitialized) aside.layout()
                    mainPane.refitTabBarShape()
                }
            }
        }
        mainPane.root.addOnLayoutChangeListener { _, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom ->
            if (right - left != oldRight - oldLeft || bottom - top != oldBottom - oldTop) {
                mainPane.root.post { mainPane.refitTabBarShape() }
            }
        }

        // Cover the home cold start until its page first renders (or times out).
        SplashOverlay.attachIfNeeded(this, appId)

        // After the cover, so the canvas can take the launch background while
        // one is on screen.
        applyCanvasBackground()

        // Get TabBar config and setup UI in parallel
        val tabBarConfig = NativeApi.getTabBarState(appId)

        // Setup containers and UI components
        mainPane.setupWebViewContainer()
        mainPane.setupTabBar(tabBarConfig)

        // Create global NavigationBar (always present, controlled by visibility)
        mainPane.createNavBar()
        aside = AsideRegion(this, rootContainer)

        // Chrome views exist now; a persisted non-default scheme can apply.
        presenterReady = true
        LxApp.setCurrentActivity(this)
        LxApp.replayStoredAppearance(this)

        // Defer capsule button creation to post-layout
        rootContainer.post {
            mainPane.addCapsuleButton()
            mainPane.reportSurfaceWidthIfNeeded()
        }

        // Setup window insets listener
        ViewCompat.setOnApplyWindowInsetsListener(rootContainer) { view, insets ->
            if (isMediaFullscreen || isPageFullscreen) {
                imeContentBottomInset = 0
                view.setPadding(0, 0, 0, 0)
                mainPane.updateLayoutMargins()
                return@setOnApplyWindowInsetsListener WindowInsetsCompat.CONSUMED
            }
            val sysBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val keyboardVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val navVisible = insets.isVisible(WindowInsetsCompat.Type.navigationBars())
            val navBottom = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            navBarHasBottomInset = navVisible && maxOf(navBottom, sysBars.bottom) > 0
            systemBottomInset = resolveContentBottomInset(insets)

            val currentBg = mainPane.tabBar?.config?.backgroundColor ?: tabBarConfig?.backgroundColor
            val isTabBarTransparent = currentBg == Color.TRANSPARENT ||
                                     (currentBg?.let { Color.alpha(it) < 255 } == true)
            // Root padding is only for an opaque TabBar that is *on screen*:
            // that strip is the bar's own plate continuing under the system
            // nav. A hidden bar must not take this path — the reserved band
            // would be empty host chrome. The WebView still stays out of the
            // gesture / navigation zone via [updateLayoutMargins].
            val hostOwnsBottom = !isTabBarTransparent && mainPane.tabBar?.visibility == View.VISIBLE &&
                mainPane.tabBar?.shape != TabBarShape.FLOATING

            // Keep host chrome anchored while resizing only the WebView around the IME.
            // Applying the IME inset to rootContainer also lifts the TabBar, unlike the
            // native iOS and Harmony shells where the keyboard covers fixed chrome.
            val baseBottom = if (hostOwnsBottom) sysBars.bottom else 0
            val sidePadLeft = if (isTabBarTransparent) 0 else sysBars.left
            val sidePadRight = if (isTabBarTransparent) 0 else sysBars.right
            view.setPadding(sidePadLeft, 0, sidePadRight, baseBottom)
            imeContentBottomInset = if (keyboardVisible) {
                (imeBottom - baseBottom).coerceAtLeast(0)
            } else {
                0
            }

            // Re-apply TabBar layout so bottom margin reflects latest inset when transparent
            mainPane.tabBar?.config?.let { cfg ->
                mainPane.tabBar?.let { tb -> mainPane.applyTabBarLayoutParams(tb, cfg) }
            }
            mainPane.updateLayoutMargins()
            mainPane.layoutCapsuleButton()
            mainPane.reportSurfaceWidthIfNeeded()
            insets
        }

        mainPane.presentFirstScreen()
        if (forceHostImmersive) {
            enterImmersiveMode()
        }

        // Defer non-critical setup to post-layout
        rootContainer.post {
            // Setup back press handler
            onBackPressedDispatcher.addCallback(object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    try {
                        // Close browser overlay first if showing (aligned with Harmony behavior)
                        if (LxAppBrowser.isShowing()) {
                            Log.d(TAG, "BackPress: dispatching to LxAppBrowser")
                            LxAppBrowser.handleBack()
                            return
                        }
                        if (LxAppSurface.closeTopUser()) {
                            Log.d(TAG, "BackPress: closing top surface")
                            return
                        }
                        // A GPU page's open dialog or popover takes back before the page leaves.
                        if (com.lingxia.view.GpuView.closeRequest(this@LxAppActivity)) return
                        // A side panel over the main is put away whole; its apps stay open.
                        if (aside.overlay) {
                            aside.collapse()
                            return
                        }
                        mainPane.currentWebView?.visibility = View.VISIBLE
                        NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_BACK_PRESS, "")
                    } catch (e: Exception) {
                        LxLog.e(TAG, "Error handling back press: ${e.message}")
                    }
                }
            })
        }
    }

    fun openWebFileChooser(
        callback: ValueCallback<Array<Uri>>?,
        params: WebChromeClient.FileChooserParams?
    ): Boolean {
        if (callback == null) {
            return false
        }

        pendingFileChooserCallback?.onReceiveValue(null)
        pendingFileChooserCallback = callback

        val intent = try {
            params?.createIntent() ?: Intent(Intent.ACTION_GET_CONTENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
            }
        } catch (error: Throwable) {
            Log.w(TAG, "create file chooser intent failed, fallback to ACTION_GET_CONTENT", error)
            Intent(Intent.ACTION_GET_CONTENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
            }
        }

        val acceptTypes = params?.acceptTypes
            ?.mapNotNull { it?.trim() }
            ?.filter { it.isNotEmpty() }
            ?: emptyList()
        val normalizedTypes = acceptTypes
            .flatMap { raw -> raw.split(',') }
            .map { it.trim() }
            .filter { it.isNotEmpty() }
        val primaryType = normalizedTypes.firstOrNull()?.let {
            if (it.startsWith(".")) "*/*" else it
        } ?: "*/*"

        intent.type = primaryType
        if (normalizedTypes.size > 1 && Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            intent.putExtra(Intent.EXTRA_MIME_TYPES, normalizedTypes.toTypedArray())
        }
        if (params?.mode == WebChromeClient.FileChooserParams.MODE_OPEN_MULTIPLE) {
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        }

        return try {
            fileChooserLauncher.launch(intent)
            true
        } catch (error: Throwable) {
            LxLog.e(TAG, "launch file chooser failed", error)
            pendingFileChooserCallback = null
            callback.onReceiveValue(null)
            false
        }
    }

    internal fun openHostFileDialog(
        intent: Intent,
        callback: (HostFileDialogResult) -> Unit,
    ): Boolean {
        if (pendingHostFileDialogCallback != null) {
            LxLog.w(TAG, "host file dialog is already open")
            return false
        }
        pendingHostFileDialogCallback = callback
        return try {
            hostFileDialogLauncher.launch(intent)
            true
        } catch (error: Throwable) {
            LxLog.e(TAG, "launch host file dialog failed", error)
            pendingHostFileDialogCallback = null
            false
        }
    }

    private fun resolveFileChooserResult(resultCode: Int, data: Intent?): Array<Uri>? {
        if (resultCode != RESULT_OK) {
            return null
        }

        val uris = mutableListOf<Uri>()
        data?.data?.let { uris.add(it) }
        val clipData = data?.clipData
        if (clipData != null) {
            for (index in 0 until clipData.itemCount) {
                clipData.getItemAt(index)?.uri?.let { uris.add(it) }
            }
        }
        return uris.distinct().takeIf { it.isNotEmpty() }?.toTypedArray()
    }

    private fun queryPickedDisplayName(uri: Uri): String? {
        return runCatching {
            contentResolver.query(
                uri,
                arrayOf(android.provider.OpenableColumns.DISPLAY_NAME),
                null,
                null,
                null,
            )?.use { cursor ->
                if (!cursor.moveToFirst()) return@use null
                val index = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                if (index >= 0) cursor.getString(index) else null
            }
        }.getOrNull()?.takeIf { it.isNotBlank() }
    }

    private fun resolveHostFileDialogResult(resultCode: Int, data: Intent?): HostFileDialogResult {
        if (resultCode == RESULT_CANCELED) {
            return HostFileDialogResult.Canceled
        }
        if (resultCode != RESULT_OK) {
            return HostFileDialogResult.Failed
        }

        // path -> display name; a local chooser path carries its name in its tail.
        val picked = linkedMapOf<String, String?>()
        // Not Map.putIfAbsent: it is API 24 and minSdk is 21.
        data?.getStringArrayListExtra("selectedPaths")?.forEach { picked.getOrPut(it) { null } }
        data?.data?.let { uri ->
            persistDocumentReadPermission(data, uri)
            picked.getOrPut(uri.toString()) { queryPickedDisplayName(uri) }
        }
        val clipData = data?.clipData
        if (clipData != null) {
            for (index in 0 until clipData.itemCount) {
                clipData.getItemAt(index)?.uri?.let { uri ->
                    persistDocumentReadPermission(data, uri)
                    picked.getOrPut(uri.toString()) { queryPickedDisplayName(uri) }
                }
            }
        }
        return if (picked.isEmpty()) {
            HostFileDialogResult.Failed
        } else {
            HostFileDialogResult.Selected(picked.keys.toList(), picked.values.toList())
        }
    }

    private fun persistDocumentReadPermission(data: Intent, uri: Uri) {
        val flags = data.flags and
            (Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        if (flags and Intent.FLAG_GRANT_READ_URI_PERMISSION == 0) {
            return
        }
        try {
            contentResolver.takePersistableUriPermission(
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION
            )
        } catch (_: Throwable) {
            // Not every provider/result supports persistable grants. The immediate grant
            // is still enough for same-session use such as sharing right after chooseFile.
        }
    }

    /**
     * Bottom inset overlays should sit above: the visible 3-button strip, or
     * the gesture-nav tap-eating zone when that strip is gone.
     */
    fun getContentBottomInset(): Int = systemBottomInset


    private fun resolveContentBottomInset(insets: WindowInsetsCompat): Int {
        return TabBarInsets.contentBottomPx(
            navVisible = insets.isVisible(WindowInsetsCompat.Type.navigationBars()),
            navBottom = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom,
            systemBarsBottom = insets.getInsets(WindowInsetsCompat.Type.systemBars()).bottom,
            mandatoryGesturesBottom = insets.getInsets(
                WindowInsetsCompat.Type.mandatorySystemGestures()
            ).bottom,
            systemGesturesBottom = insets.getInsets(
                WindowInsetsCompat.Type.systemGestures()
            ).bottom,
        )
    }
    override fun onResume() {
        super.onResume()
        mainPane.webViewContainer.visibility = View.VISIBLE
        // On a cold start the first screen is still being presented; it attaches itself.
        mainPane.currentWebView?.let(mainPane::attachWebViewToUI)
        if (::aside.isInitialized) aside.onHostPaused(false)
        val currentPath = mainPane.currentWebView?.getCurrentPath()
        if (!currentPath.isNullOrEmpty()) {
            applyPageOrientation(currentPath)
        }
        if (forceHostImmersive) {
            enterImmersiveMode()
        }
        // Resume native components
        mainPane.currentWebView?.let { (it as? WebView)?.let { NativeBridge.notifyPageActive(it) } }
        // Show the "ready to install" prompt for an update that finished
        // downloading while no activity was in the foreground.
        UpdateManager.tryShowPendingReadyInstall()
        UpdateManager.tryShowPendingStoreUpdate()
        // Resume any update install that was deferred while the user was in
        // the "Install unknown apps" system settings screen.
        UpdateManager.tryInstallPendingUpdate()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && (isPageFullscreen || forceHostImmersive)) {
            enterImmersiveMode()
        }
    }

    /**
     * A tap right after a back is for the page it returns to, which is not shown yet: the
     * gesture is held and replayed to it once shown, rather than landing on the cover or on
     * the page being left.
     */
    override fun dispatchTouchEvent(event: android.view.MotionEvent): Boolean {
        val down = event.actionMasked == android.view.MotionEvent.ACTION_DOWN
        if (!replayingTouches && (backHold != 0L || replayQueued) && (holdingGesture || down)) {
            if (!holdingGesture) LxLog.i(NAV_TAG, "touch held at ${event.x.toInt()},${event.y.toInt()} until the page a back returns to shows")
            holdingGesture = true
            if (heldTouches.size < 512) heldTouches.add(android.view.MotionEvent.obtain(event))
            return true
        }
        // A finger during a slide is for the page sliding in, which is not where it will be:
        // the slide ends first so the touch lands on the page as it is shown.
        if (down) mainPane.slideFinish?.let { finish ->
            LxLog.i(NAV_TAG, "touch at ${event.x.toInt()},${event.y.toInt()} during a slide; ending it first")
            finish.run()
        }
        val taken = super.dispatchTouchEvent(event)
        if (down && !taken) {
            LxLog.i(NAV_TAG, "touch at ${event.x.toInt()},${event.y.toInt()} taken by no view" +
                if (replayingTouches) " (replayed)" else "")
        }
        return taken
    }

    /** Ends the hold of back navigation [version] (any, when null) and replays what it held. */
    internal fun releaseBackHold(version: Long? = null) {
        if (backHold == 0L || (version != null && version != backHold)) return
        backHold = 0L
        if (heldTouches.isEmpty()) {
            holdingGesture = false
            return
        }
        // The rest of a held gesture keeps queueing until the replay: sent live, its up would
        // reach the page before the replayed down.
        replayQueued = true
        // Posted: the page shown in this turn takes the gesture after it is laid out.
        window.decorView.post {
            val held = ArrayList(heldTouches)
            heldTouches.clear()
            replayQueued = false
            holdingGesture = false
            LxLog.i(NAV_TAG, "replaying ${held.size} held touch events")
            replayingTouches = true
            try {
                held.forEach { dispatchTouchEvent(it) }
            } finally {
                replayingTouches = false
                held.forEach { it.recycle() }
            }
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (hasAppId && appId.isNotBlank()) {
            val action = event.action
            if (action == KeyEvent.ACTION_DOWN || action == KeyEvent.ACTION_UP) {
                val payload = buildKeyEventPayload(event)
                if (payload != null) {
                    val eventType = if (action == KeyEvent.ACTION_DOWN) {
                        NativeApi.KEY_EVENT_DOWN
                    } else {
                        NativeApi.KEY_EVENT_UP
                    }
                    runCatching {
                        NativeApi.onKeyEvent(appId, eventType, payload)
                    }.onFailure { error ->
                        Log.w(TAG, "onKeyEvent failed: ${error.message}")
                    }
                }
            }
        }
        return super.dispatchKeyEvent(event)
    }

    private fun buildKeyEventPayload(event: KeyEvent): String? {
        val code = KeyEvent.keyCodeToString(event.keyCode)
        val key = resolveKey(event, code)
        if (key.isEmpty() && code.isEmpty()) return null

        return JSONObject().apply {
            put("key", key)
            put("code", code)
            if (event.isAltPressed) put("altKey", true)
            if (event.isCtrlPressed) put("ctrlKey", true)
            if (event.isShiftPressed) put("shiftKey", true)
            if (event.isMetaPressed) put("metaKey", true)
            if (event.repeatCount > 0) put("repeat", true)
        }.toString()
    }

    private fun resolveKey(event: KeyEvent, code: String): String {
        val unicode = event.getUnicodeChar(event.metaState)
        if (unicode != 0) {
            return String(Character.toChars(unicode))
        }

        return when (event.keyCode) {
            KeyEvent.KEYCODE_ENTER -> "Enter"
            KeyEvent.KEYCODE_DEL -> "Backspace"
            KeyEvent.KEYCODE_TAB -> "Tab"
            KeyEvent.KEYCODE_ESCAPE -> "Escape"
            KeyEvent.KEYCODE_BACK -> "Back"
            KeyEvent.KEYCODE_DPAD_LEFT -> "ArrowLeft"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "ArrowRight"
            KeyEvent.KEYCODE_DPAD_UP -> "ArrowUp"
            KeyEvent.KEYCODE_DPAD_DOWN -> "ArrowDown"
            KeyEvent.KEYCODE_SPACE -> " "
            KeyEvent.KEYCODE_MOVE_HOME -> "Home"
            KeyEvent.KEYCODE_MOVE_END -> "End"
            KeyEvent.KEYCODE_PAGE_UP -> "PageUp"
            KeyEvent.KEYCODE_PAGE_DOWN -> "PageDown"
            else -> code.removePrefix("KEYCODE_")
        }
    }

    override fun onPause() {
        super.onPause()
        mainPane.currentWebView?.pause()
        mainPane.currentWebView?.let { (it as? WebView)?.let { NativeBridge.notifyPageInactive(it) } }
        if (::aside.isInitialized) aside.onHostPaused(true)
    }

    override fun onStart() {
        super.onStart()

        if (hasAppId && appId.isNotBlank()) {
            mainPane.updateCapsuleButtonVisibility(appId)
            if (hasEnteredBackground) {
                NativeApi.onAppShow(appId)
                hasEnteredBackground = false
            }
        }

    }

    override fun onStop() {
        super.onStop()

        // Avoid spurious background/foreground events during configuration changes (e.g. rotation).
        if (isChangingConfigurations) return

        if (hasAppId && appId.isNotBlank()) {
            NativeApi.onAppHide(appId)
            hasEnteredBackground = true
        }
    }

    /**
     * Notifies the native layer that the LxApp is being closed
     * Returns whether the close matches current runtime session.
     */
    internal lateinit var aside: AsideRegion

    fun contentRoot(): FrameLayout = rootContainer

    internal fun hasAsideRegion(): Boolean = ::aside.isInitialized

    internal fun asideDocked(): Boolean = ::aside.isInitialized && aside.docked && !isMediaFullscreen

    /** The pane showing [appId]: the main's, or a side panel tab's. */
    internal fun paneOf(appId: String): LxAppPane? = when {
        hasAppId && mainPane.appId == appId -> mainPane
        ::aside.isInitialized -> aside.panes[appId]
        else -> null
    }

    /** Opens [appId] in the side panel; the core's next plan places it. */
    internal fun openAside(appId: String, sessionId: Long) {
        if (paneOf(appId) != null) return
        aside.open(appId, sessionId)
    }

    internal fun dropAside(appId: String) {
        aside.drop(appId)
    }

    /** The main gives a docked aside its room. */
    internal fun onAsideLayoutChanged() {
        val right = if (asideDocked()) aside.dockedWidth() else 0
        (mainPane.root.layoutParams as? FrameLayout.LayoutParams)?.let { lp ->
            if (lp.rightMargin != right) {
                lp.rightMargin = right
                mainPane.root.layoutParams = lp
            }
        }
        if (mainPane.hasWebViewContainer) mainPane.updateLayoutMargins()
    }

    /**
     * The core's plan for the window. The side panel follows its `panel` slot, and an lxapp the
     * plan moved between main and aside is re-hosted: its pages stay alive in the runtime, so
     * the move shows the page it was on.
     */
    internal fun applyLayoutPlan(plan: org.json.JSONObject) {
        if (!::aside.isInitialized || !hasAppId) return
        val slot = AsideRegion.Slot.parse(plan)
        val activeMain = activeMainAppId(plan)
        val current = mainPane.appId
        if (activeMain != null && activeMain != current && aside.panes.containsKey(activeMain)) {
            // Promoted: the aside's pages go to the main.
            aside.drop(activeMain)
            showInMain(activeMain)
        } else if (current.isNotBlank() && slot?.children?.contains(current) == true) {
            // Demoted: the main's pages go to the side panel; the successor takes the main.
            val session = mainPane.currentSessionId
            activeMain?.takeIf { it != current }?.let(::showInMain)
            aside.open(current, session)
        }
        aside.apply(slot)
    }

    private fun showInMain(appId: String) {
        val session = NativeApi.getLxAppSessionId(appId)
        if (session <= 0L) return
        if (NativeApi.onLxAppPageOpened(appId, "", session).isNotBlank()) mainPane.openLxApp(appId, session)
    }

    private fun activeMainAppId(plan: org.json.JSONObject): String? {
        val id = plan.optString("activeMainId").takeIf { it.isNotEmpty() && !plan.isNull("activeMainId") }
            ?: return null
        val items = plan.optJSONObject("mainSwitcher")?.optJSONArray("items") ?: return id
        for (index in 0 until items.length()) {
            val item = items.optJSONObject(index) ?: continue
            if (item.optString("surfaceId") != id) continue
            val content = item.optJSONObject("content") ?: return null
            return content.optString("appId").takeIf { content.optString("kind") == "lxapp" && it.isNotEmpty() }
        }
        return id
    }

    internal fun beginBackHold(version: Long) {
        backHold = version
        // Bounded: whatever shows by then takes the held gesture.
        window.decorView.postDelayed({ releaseBackHold(version) }, BACK_HOLD_MS)
    }

    internal fun navigate(webtag: String?, animationType: AnimationType): Boolean =
        mainPane.navigate(webtag, animationType)
    fun closeLxApp(sessionId: Long = currentSessionId) = mainPane.closeLxApp(sessionId)

    /** Closes [appId] wherever it shows; an aside's slot lets it go. */
    internal fun closeLxApp(appId: String, sessionId: Long) {
        if (::aside.isInitialized && aside.panes.containsKey(appId)) {
            if (aside.panes[appId]?.currentSessionId == sessionId) aside.close(appId)
            return
        }
        mainPane.closeLxApp(sessionId)
    }
    fun openLxApp(appId: String, sessionId: Long) = mainPane.openLxApp(appId, sessionId)
    fun getAppId(): String = appId
    fun getSessionId(): Long = currentSessionId
    internal fun getCurrentPageWebTag(): String? = mainPane.getCurrentPageWebTag()
    internal fun getCurrentWebView(): com.lingxia.lxapp.WebView? = mainPane.getCurrentWebView()
    fun preserveGpuPageBeforeDisposal(page: GpuPage, release: Runnable) =
        mainPane.preserveGpuPageBeforeDisposal(page, release)
    internal fun applyCanvasBackground() = mainPane.applyCanvasBackground()
    fun getCapsuleRectJSON(requestingAppId: String): String = mainPane.getCapsuleRectJSON(requestingAppId)

    override fun onDestroy() {
        isDestroyed = true
        cancelOrientationRelease()
        if (::mainPane.isInitialized) mainPane.destroy()
        pendingFileChooserCallback?.onReceiveValue(null)
        pendingFileChooserCallback = null
        pendingHostFileDialogCallback?.invoke(HostFileDialogResult.Failed)
        pendingHostFileDialogCallback = null

        // A replacement activity can already be active when Android destroys
        // this instance during a configuration change.
        LxApp.clearCurrentActivity(this)

        super.onDestroy()
    }

    fun enterMediaFullscreen() {
        if (isMediaFullscreen) return
        val navBar = mainPane.navigationBar
        val tab = mainPane.tabBar
        val navParent = navBar?.parent as? ViewGroup
        val navIndex = navParent?.indexOfChild(navBar) ?: -1
        val overlayHost = rootContainer.findViewWithTag<View>("ComponentOverlay")
        val overlayParams = overlayHost?.layoutParams as? FrameLayout.LayoutParams
        mediaFullscreenState = MediaFullscreenState(
            tabBarVisibility = tab?.visibility ?: View.GONE,
            navigationBarVisibility = navBar?.visibility ?: View.GONE,
            navigationBarIndex = navIndex,
            navigationBarLayoutParams = navBar?.layoutParams,
            overlayLayoutParams = overlayParams?.let { FrameLayout.LayoutParams(it) },
            overlayTranslationX = overlayHost?.translationX ?: 0f,
            overlayTranslationY = overlayHost?.translationY ?: 0f,
            rootPaddingLeft = rootContainer.paddingLeft,
            rootPaddingTop = rootContainer.paddingTop,
            rootPaddingRight = rootContainer.paddingRight,
            rootPaddingBottom = rootContainer.paddingBottom
        )
        pendingTabBarVisibility = null
        pendingNavBarVisibility = null
        imeContentBottomInset = 0
        isMediaFullscreen = true
        if (overlayHost != null && rootContainer.indexOfChild(overlayHost) != rootContainer.childCount - 1) {
            shouldRestoreOverlayOrder = true
            rootContainer.bringChildToFront(overlayHost)
        }
        if (overlayHost != null) {
            overlayHost.layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            ).apply {
                leftMargin = 0
                topMargin = 0
                rightMargin = 0
                bottomMargin = 0
            }
            overlayHost.translationX = 0f
            overlayHost.translationY = 0f
            overlayHost.requestLayout()
        }
        if (mainPane.root.indexOfChild(mainPane.webViewContainer) != mainPane.root.childCount - 1) {
            shouldRestoreOverlayOrder = true
            mainPane.root.bringChildToFront(mainPane.webViewContainer)
        }
        if (navParent != null && navIndex >= 0) {
            navParent.removeView(navBar)
        } else {
            navBar?.visibility = View.GONE
        }
        navBar?.visibility = View.GONE
        tab?.visibility = View.GONE
        mainPane.updateCapsuleButton()
        rootContainer.setPadding(0, 0, 0, 0)
        mainPane.updateLayoutMargins()
        rootContainer.requestApplyInsets()
    }

    fun exitMediaFullscreen() {
        if (!isMediaFullscreen) return
        isMediaFullscreen = false
        mediaFullscreenState?.let { state ->
            val tabVisibility = pendingTabBarVisibility ?: state.tabBarVisibility
            val navVisibility = pendingNavBarVisibility ?: state.navigationBarVisibility
            mainPane.tabBar?.visibility = tabVisibility
            mainPane.navigationBar?.visibility = navVisibility
            mainPane.syncNavigationBarToTabBar(visible = tabVisibility == View.VISIBLE)
            rootContainer.setPadding(
                state.rootPaddingLeft,
                state.rootPaddingTop,
                state.rootPaddingRight,
                state.rootPaddingBottom
            )
            val navBar = mainPane.navigationBar
            if (navBar != null && navBar.parent == null && state.navigationBarIndex >= 0) {
                val params = state.navigationBarLayoutParams ?: FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT
                )
                mainPane.root.addView(navBar, state.navigationBarIndex, params)
                navBar.visibility = navVisibility
            }
            val overlayHost = rootContainer.findViewWithTag<View>("ComponentOverlay")
            if (overlayHost != null) {
                state.overlayLayoutParams?.let { overlayHost.layoutParams = FrameLayout.LayoutParams(it) }
                overlayHost.translationX = state.overlayTranslationX
                overlayHost.translationY = state.overlayTranslationY
                overlayHost.requestLayout()
            }
        }
        mediaFullscreenState = null
        pendingTabBarVisibility = null
        pendingNavBarVisibility = null
        mainPane.updateCapsuleButton()
        if (shouldRestoreOverlayOrder) {
            mainPane.navigationBar?.let { mainPane.root.bringChildToFront(it) }
            mainPane.tabBar?.let { mainPane.root.bringChildToFront(it) }
            statusBand?.let { mainPane.root.bringChildToFront(it) }
            mainPane.root.findViewWithTag<View>("capsule_button")?.let { mainPane.root.bringChildToFront(it) }
            shouldRestoreOverlayOrder = false
        }
        mainPane.updateLayoutMargins()
        rootContainer.requestApplyInsets()
        // Ensure WebView top margin is recalculated after NavigationBar has been re-attached/measured.
        rootContainer.post {
            if (isMediaFullscreen) return@post
            mainPane.updateLayoutMargins()
            rootContainer.requestApplyInsets()
        }
    }

    /**
     * Apply page orientation configuration
     */
    internal fun applyPageOrientation(path: String) {
        val normalizedPath = mainPane.normalizePath(path)
        val orientation = NativeApi.getPageOrientation(appId, normalizedPath)

        updateRequestedOrientation(orientation)
        updateImmersiveMode(orientation, normalizedPath)
        dispatchUiOrientationChangeIfNeeded()
    }

    /** Reading `requestedOrientation` is a synchronous system call; only this activity sets it. */
    private var appliedOrientation: Int? = null

    /** The window's orientation when a forced one (a full-screen video's) replaced a free one. */
    private var orientationBeforeForce: Int? = null
    /** Lifting a force waits for the window to turn back before following the sensor again. */
    private var pendingRelease: Runnable? = null
    private val orientationHandler = android.os.Handler(android.os.Looper.getMainLooper())

    /** What the host app's manifest declares for this activity: where a page without its own lands. */
    private val hostOrientation: Int by lazy {
        runCatching { packageManager.getActivityInfo(componentName, 0).screenOrientation }
            .getOrDefault(ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED)
    }

    private fun updateRequestedOrientation(orientation: Int) {
        val targetOrientation = resolveScreenOrientation(orientation)
        if (appliedOrientation == targetOrientation) return
        val free = hostOrientation
        val wasFree = (appliedOrientation ?: free) == free
        appliedOrientation = targetOrientation
        val returning = pendingRelease != null
        cancelOrientationRelease()
        if (targetOrientation != free) {
            // A force during a return keeps the orientation recorded before the first one.
            if (wasFree && !returning) orientationBeforeForce = windowOrientation()
            requestedOrientation = targetOrientation
            return
        }
        val before = orientationBeforeForce
        if (before == null || before == windowOrientation()) {
            orientationBeforeForce = null
            requestedOrientation = free
            return
        }
        // Releasing at once would keep the forced rotation on a phone lying flat: the sensor
        // reports nothing new. Turn back first, then follow the sensor.
        requestedOrientation = before
        val release = Runnable {
            pendingRelease = null
            orientationBeforeForce = null
            requestedOrientation = free
        }
        pendingRelease = release
        orientationHandler.postDelayed(release, ORIENTATION_RETURN_TIMEOUT_MS)
    }

    private fun cancelOrientationRelease() {
        pendingRelease?.let(orientationHandler::removeCallbacks)
        pendingRelease = null
    }

    /** A pending release runs as soon as the window has turned back. */
    private fun releaseOrientationIfReturned() {
        val release = pendingRelease ?: return
        if (orientationBeforeForce == windowOrientation()) {
            orientationHandler.removeCallbacks(release)
            release.run()
        }
    }

    private fun windowOrientation(): Int =
        if (resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE) {
            ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        } else {
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        }

    private fun resolveScreenOrientation(orientation: Int): Int {
        return when (orientation) {
            NativeApi.ORIENTATION_PORTRAIT -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            NativeApi.ORIENTATION_LANDSCAPE -> ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            NativeApi.ORIENTATION_REVERSE_PORTRAIT -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT
            NativeApi.ORIENTATION_REVERSE_LANDSCAPE -> ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE
            else -> hostOrientation
        }
    }

    private fun updateImmersiveMode(orientation: Int, path: String) {
        val shouldFullscreen = shouldEnterImmersiveMode(orientation, path)
        if (shouldFullscreen) {
            // Re-entering rewrites window flags and costs a WM relayout per switch.
            if (!isPageFullscreen) enterImmersiveMode()
        } else {
            exitImmersiveMode()
        }
    }

    private fun shouldEnterImmersiveMode(orientation: Int, path: String): Boolean {
        if (forceHostImmersive) {
            return true
        }

        if (isTelevisionDevice()) {
            return true
        }

        if (
            orientation != NativeApi.ORIENTATION_LANDSCAPE &&
            orientation != NativeApi.ORIENTATION_REVERSE_LANDSCAPE
        ) {
            return false
        }

        val navbarState = NativeApi.getNavigationBarState(appId, path)
        return navbarState != null && !navbarState.showNavbar
    }

    /**
     * Enter immersive fullscreen mode (hide status bar and navigation bar)
     */
    private fun enterImmersiveMode() {
        isPageFullscreen = true
        imeContentBottomInset = 0

        // Allow content to extend into display cutout (notch/punch hole) area
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }

        @Suppress("DEPRECATION")
        window.setFlags(
            android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN,
            android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN
        )
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        controller?.apply {
            hide(WindowInsetsCompat.Type.statusBars())
            hide(WindowInsetsCompat.Type.navigationBars())
            hide(WindowInsetsCompat.Type.displayCutout())
            systemBarsBehavior = androidx.core.view.WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        applyLegacyImmersiveFlags()
        // Trigger layout update
        rootContainer.setPadding(0, 0, 0, 0)
        mainPane.updateLayoutMargins()
        rootContainer.requestApplyInsets()
    }

    /**
     * Exit immersive mode (show status bar and navigation bar)
     */
    private fun exitImmersiveMode() {
        if (!isPageFullscreen) {
            return
        }
        isPageFullscreen = false

        // Restore default cutout mode
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_DEFAULT
        }

        @Suppress("DEPRECATION")
        window.clearFlags(android.view.WindowManager.LayoutParams.FLAG_FULLSCREEN)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        controller?.apply {
            show(WindowInsetsCompat.Type.statusBars())
            show(WindowInsetsCompat.Type.navigationBars())
        }
        clearLegacyImmersiveFlags()
        // Trigger layout update
        rootContainer.requestApplyInsets()
    }

    private fun isTelevisionDevice(): Boolean {
        val uiModeType = resources.configuration.uiMode and Configuration.UI_MODE_TYPE_MASK
        if (uiModeType == Configuration.UI_MODE_TYPE_TELEVISION) {
            return true
        }

        val uiModeManager = getSystemService(Context.UI_MODE_SERVICE) as? UiModeManager
        if (uiModeManager?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION) {
            return true
        }

        val isLeanbackOnly = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK_ONLY)
        return packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK) || isLeanbackOnly
    }

    private fun isHostImmersiveEnabled(): Boolean {
        return try {
            val appInfo = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                packageManager.getApplicationInfo(
                    packageName,
                    android.content.pm.PackageManager.ApplicationInfoFlags.of(
                        android.content.pm.PackageManager.GET_META_DATA.toLong()
                    )
                )
            } else {
                @Suppress("DEPRECATION")
                packageManager.getApplicationInfo(
                    packageName,
                    android.content.pm.PackageManager.GET_META_DATA
                )
            }
            appInfo.metaData?.getBoolean(META_FORCE_IMMERSIVE, false) == true
        } catch (error: Throwable) {
            false
        }
    }

    @Suppress("DEPRECATION")
    private fun applyLegacyImmersiveFlags() {
        window.decorView.systemUiVisibility =
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
                View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_FULLSCREEN or
                View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE
    }

    @Suppress("DEPRECATION")
    private fun clearLegacyImmersiveFlags() {
        window.decorView.systemUiVisibility =
            View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
                View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE
    }

    // Handle configuration changes to prevent Activity recreation
    override fun onConfigurationChanged(newConfig: android.content.res.Configuration) {
        super.onConfigurationChanged(newConfig)
        releaseOrientationIfReturned()

        NativeApi.onHostAppearanceChanged()

        // Update layout to adapt to screen orientation changes
        if (::mainPane.isInitialized && mainPane.hasWebViewContainer) {
            mainPane.updateLayoutMargins()
            // Kept GPU pages take their own size in the new window once it is laid out, so a
            // hidden page's catch-up frame is the one it will show (frameIsCurrent).
            mainPane.webViewContainer.viewTreeObserver.addOnGlobalLayoutListener(object :
                android.view.ViewTreeObserver.OnGlobalLayoutListener {
                override fun onGlobalLayout() {
                    mainPane.webViewContainer.viewTreeObserver.removeOnGlobalLayoutListener(this)
                    mainPane.layoutKeptPages()
                }
            })
        }
        rootContainer.post { mainPane.reportSurfaceWidthIfNeeded() }
        // A display-size change keeps the window's pixels but not its dp width.
        rootContainer.post { mainPane.refitTabBarShape() }
        dispatchUiOrientationChangeIfNeeded()
    }

    private fun currentUiOrientationLabel(): String? {
        return when (resources.configuration.orientation) {
            android.content.res.Configuration.ORIENTATION_LANDSCAPE -> "landscape"
            android.content.res.Configuration.ORIENTATION_PORTRAIT -> "portrait"
            else -> null
        }
    }

    private fun dispatchUiOrientationChangeIfNeeded() {
        val orientationValue = currentUiOrientationLabel() ?: return
        if (lastDispatchedDeviceOrientation == orientationValue) {
            return
        }
        if (!hasAppId || appId.isBlank()) {
            return
        }
        val sessionId = currentSessionId
        if (sessionId <= 0L) {
            return
        }

        try {
            if (NativeApi.onDeviceOrientationChanged(appId, sessionId, orientationValue)) {
                lastDispatchedDeviceOrientation = orientationValue
            }
        } catch (error: Throwable) {
            Log.w(TAG, "onDeviceOrientationChanged failed: ${error.message}")
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        if (PermissionManager.handleRequestPermissionsResult(requestCode, permissions, grantResults)) {
            return
        }
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    }
}
