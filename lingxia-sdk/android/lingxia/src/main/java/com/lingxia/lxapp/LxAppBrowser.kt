package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.InputType
import android.text.TextUtils
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.lingxia.app.Lingxia
import com.lingxia.app.NativeApi
import com.lingxia.webview.LingXiaWebView
import java.net.URI

internal object LxAppBrowser {
    private const val TAG = "LingXia.Browser"
    private const val ATTACH_RETRY_DELAY_MS = 100L
    private const val ATTACH_MAX_RETRIES = 8
    private const val HIDDEN_NEW_TAB_URL = "lingxia://newtab"

    private val openTabIds = mutableListOf<String>()
    private var activeTabId: String? = null
    private var pendingTabId: String? = null
    private var pendingAttachToken: Long = 0L

    private var overlayContainer: FrameLayout? = null
    private var contentHost: FrameLayout? = null
    private var bottomBar: View? = null
    private var tabSwitcher: View? = null
    private var overflowMenu: View? = null
    private var activeWebView: LingXiaWebView? = null
    private var activeWebViewTabId: String? = null
    private var currentActivity: Activity? = null

    private var addressIcon: ImageView? = null
    private var addressField: EditText? = null
    private var addressRow: View? = null
    private var backButton: ImageView? = null
    private var forwardButton: ImageView? = null
    private var asideRefreshButton: View? = null
    private var plusButton: View? = null
    private var menuButton: View? = null
    private var tabsBadge: TextView? = null
    // Aside tabs are a distinct API-managed group. Their compact projection is
    // a one-row toolbar without address editing or user tab creation.
    private var isAsideActive = false
    // Chrome-style history intervention: until the user interacts with a tab
    // (page touch or address navigation), auto-created history (SPA pushState
    // redirects) must not light back/forward.
    private val interactedTabIds = mutableSetOf<String>()

    /** Chrome colors; the light set is the original design. */
    private data class Palette(
        val dark: Boolean,
        val canvas: Int,
        val bar: Int,
        val stroke: Int,
        val pill: Int,
        val icon: Int,
        val secondaryIcon: Int,
        val text: Int,
        val hint: Int,
        val warning: Int,
        val surface: Int,
        val activeRow: Int,
        val title: Int,
        val scrim: Int
    )

    private val lightPalette = Palette(
        dark = false,
        canvas = Color.WHITE,
        bar = Color.parseColor("#FAFFFFFF"),
        stroke = Color.parseColor("#14000000"),
        pill = Color.parseColor("#F0F0F0"),
        icon = Color.parseColor("#333333"),
        secondaryIcon = Color.parseColor("#666666"),
        text = Color.parseColor("#333333"),
        hint = Color.parseColor("#888888"),
        warning = Color.parseColor("#C44A21"),
        surface = Color.WHITE,
        activeRow = Color.parseColor("#F2F4F7"),
        title = Color.parseColor("#222222"),
        scrim = Color.parseColor("#66000000")
    )

    private val darkPalette = Palette(
        dark = true,
        canvas = Color.parseColor("#1C1C1E"),
        bar = Color.parseColor("#FA242426"),
        stroke = Color.parseColor("#1FFFFFFF"),
        pill = Color.parseColor("#3A3A3C"),
        icon = Color.parseColor("#E5E5E7"),
        secondaryIcon = Color.parseColor("#A1A1AA"),
        text = Color.parseColor("#F2F2F7"),
        hint = Color.parseColor("#8E8E93"),
        warning = Color.parseColor("#FF9F43"),
        surface = Color.parseColor("#2C2C2E"),
        activeRow = Color.parseColor("#3A3A3C"),
        title = Color.parseColor("#F2F2F7"),
        scrim = Color.parseColor("#A6000000")
    )

    private var palette = lightPalette

    private val chromeRefreshRunnable = object : Runnable {
        override fun run() {
            refreshChromeFromActiveWebView()
            scheduleChromeRefresh()
        }
    }

    fun show(activity: Activity, tabId: String, initialUrl: String = ""): Boolean {
        val normalizedTabId = normalizeTabId(tabId)
        if (normalizedTabId.isEmpty()) {
            Log.w(TAG, "show failed: empty tabId")
            return false
        }

        registerTab(normalizedTabId)
        val tabChanged = activeTabId != normalizedTabId
        activeTabId = normalizedTabId
        currentActivity = activity
        NativeApi.browserTabActivate(normalizedTabId)

        if (!ensureChrome(activity)) {
            return false
        }
        applyAppearance(activity)
        if (tabChanged) {
            onActiveTabSwitched(activity, normalizedTabId)
        } else {
            applyActiveModeChrome(activity)
        }
        closeOverflowMenu()
        closeTabSwitcher()
        startChromeRefreshLoop()
        beginAttachActiveTab(activity, initialUrl.trim())
        return true
    }

    fun dismiss() {
        pendingAttachToken += 1
        pendingTabId = null
        stopChromeRefreshLoop()
        closeOverflowMenu()
        closeTabSwitcher()
        currentActivity?.let(::releaseBarGlyphs)

        activeWebView?.pause()
        activeWebView?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        activeWebView = null
        activeWebViewTabId = null

        overlayContainer?.let { container ->
            ViewCompat.setOnApplyWindowInsetsListener(container, null)
            (container.parent as? ViewGroup)?.removeView(container)
        }
        overlayContainer = null
        contentHost = null
        bottomBar = null
        currentActivity = null
        addressIcon = null
        addressField = null
        addressRow = null
        plusButton = null
        menuButton = null
        backButton = null
        forwardButton = null
        asideRefreshButton = null
        tabsBadge = null
    }

    fun isShowing(): Boolean = overlayContainer != null

    /**
     * An lxapp's scheme was applied. The browser follows its own built-in
     * app — the product's scheme — and the host lxapp's apply may have just
     * restyled the system bars underneath it, so re-assert either way.
     */
    fun onAppearanceChanged() {
        val container = overlayContainer ?: return
        container.post {
            val activity = currentActivity ?: return@post
            if (overlayContainer === container) applyAppearance(activity)
        }
    }

    private fun resolveDark(): Boolean {
        val browserAppId = runCatching { NativeApi.getBuiltinBrowserAppId() }.getOrNull()
        return browserAppId?.let(LxApp::appearanceDarkFor) ?: LxApp.hostAppearanceDark()
    }

    private fun applyAppearance(activity: Activity) {
        val next = if (resolveDark()) darkPalette else lightPalette
        if (next != palette) {
            palette = next
            restyleChrome(activity)
            applyTabsAppearance()
        }
        assertBarGlyphs(activity)
    }

    /**
     * Tab WebViews take prefers-color-scheme from their creation context, and
     * nothing re-dispatches a configuration to them while they live, so hand
     * every open tab one carrying the current night bit.
     */
    private fun applyTabsAppearance() {
        val night = if (palette.dark) {
            Configuration.UI_MODE_NIGHT_YES
        } else {
            Configuration.UI_MODE_NIGHT_NO
        }
        for (tabId in openTabIds) {
            val webView = findManagedWebView(tabId) ?: continue
            val config = Configuration(webView.resources.configuration)
            config.uiMode = (config.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or night
            webView.dispatchConfigurationChanged(config)
        }
    }

    /** Rebuild the bar in the new palette; transient overlays just close. */
    private fun restyleChrome(activity: Activity) {
        val container = overlayContainer ?: return
        closeOverflowMenu()
        closeTabSwitcher()
        container.setBackgroundColor(palette.canvas)
        val oldBar = bottomBar ?: return
        val typing = addressField?.takeIf { it.hasFocus() }?.text?.toString()
        val bar = buildBottomBar(activity, activity.resources.displayMetrics.density)
        val index = container.indexOfChild(oldBar)
        container.removeView(oldBar)
        container.addView(bar, index)
        bottomBar = bar
        applyActiveModeChrome(activity)
        if (typing != null) {
            addressField?.setText(typing)
        } else {
            refreshChromeFromActiveWebView()
        }
    }

    private fun assertBarGlyphs(activity: Activity) {
        val light = !palette.dark
        WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
            isAppearanceLightStatusBars = light
            isAppearanceLightNavigationBars = light
        }
    }

    /**
     * Hand the bars back by recomputing them from the lxapp underneath, so
     * anything it changed while covered is reflected.
     */
    private fun releaseBarGlyphs(activity: Activity) {
        val lxActivity = activity as? LxAppActivity ?: return
        val dark = (activity.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) ==
            Configuration.UI_MODE_NIGHT_YES
        // Navigation-bar glyphs follow the activity's scheme
        // (LxAppActivity.configureTransparentSystemBars); the status bar is
        // the current page's to decide.
        WindowCompat.getInsetsController(activity.window, activity.window.decorView)
            .isAppearanceLightNavigationBars = !dark
        LxAppActivity.updateNavBarUI(lxActivity.getAppId())
    }

    fun handleBack(): Boolean {
        if (!isShowing()) {
            return false
        }
        if (overflowMenu != null) {
            closeOverflowMenu()
            return true
        }
        if (tabSwitcher != null) {
            closeTabSwitcher()
            return true
        }
        // Browser history has its own toolbar button. Back on an aside exits
        // the full-screen slot; self mode keeps the familiar history-first
        // behavior.
        if (!isAsideActive && activeWebView?.canGoBack() == true) {
            navigateBack()
            return true
        }
        dismiss()
        return true
    }

    private fun ensureChrome(activity: Activity): Boolean {
        val existing = overlayContainer
        if (existing != null) {
            currentActivity = activity
            return true
        }

        val rootView = activity.window.decorView as? ViewGroup ?: return false
        val density = activity.resources.displayMetrics.density
        palette = if (resolveDark()) darkPalette else lightPalette

        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(palette.canvas)
            fitsSystemWindows = false
            clipChildren = false
            clipToPadding = false
        }

        val host = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }

        val bar = buildBottomBar(activity, density)
        container.addView(host)
        container.addView(bar)
        rootView.addView(container)

        overlayContainer = container
        contentHost = host
        bottomBar = bar
        currentActivity = activity

        ViewCompat.setOnApplyWindowInsetsListener(container) { _, insets ->
            val bar = bottomBar ?: return@setOnApplyWindowInsetsListener insets
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            val keyboardVisible = insets.isVisible(WindowInsetsCompat.Type.ime())
            val barHeight = dp(activity, currentBarHeightDp())
            val navInset = if (keyboardVisible) 0 else systemBars.bottom
            val liftInset = if (keyboardVisible) ime.bottom else 0
            val totalBarHeight = barHeight + navInset

            (host.layoutParams as? FrameLayout.LayoutParams)?.let { params ->
                params.topMargin = systemBars.top
                params.bottomMargin = totalBarHeight + liftInset
                host.layoutParams = params
            }
            (bar.layoutParams as? FrameLayout.LayoutParams)?.let { params ->
                params.height = totalBarHeight
                params.bottomMargin = liftInset
                bar.layoutParams = params
            }
            bar.setPadding(dp(activity, 12), dp(activity, 6), dp(activity, 12), dp(activity, 6) + navInset)
            insets
        }
        ViewCompat.requestApplyInsets(container)
        // Tabs outlive the chrome; the scheme may have moved while it was gone.
        applyTabsAppearance()
        return true
    }

    private fun buildBottomBar(activity: Activity, density: Float): View {
        val barHeight = dp(activity, 96)
        val bar = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                barHeight
            ).apply {
                gravity = Gravity.BOTTOM
                bottomMargin = 0
            }
            background = GradientDrawable().apply {
                setColor(palette.bar)
                cornerRadius = 0f
                setStroke(maxOf(1, (0.5f * density).toInt()), palette.stroke)
            }
            elevation = 0f
            setPadding(dp(activity, 12), dp(activity, 6), dp(activity, 12), dp(activity, 6))
        }

        val addressRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        val addressPill = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.MATCH_PARENT,
                1f
            )
            background = GradientDrawable().apply {
                setColor(palette.pill)
                cornerRadius = dp(activity, 18).toFloat()
            }
            setPadding(dp(activity, 12), 0, dp(activity, 4), 0)
        }

        val addrIcon = ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, 18), dp(activity, 18)).apply {
                rightMargin = dp(activity, 6)
            }
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageResource(R.drawable.icon_lock)
            setColorFilter(palette.secondaryIcon)
            isFocusable = false
            isClickable = false
        }
        val addrField = EditText(activity).apply {
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 14f)
            setTextColor(palette.text)
            setHintTextColor(palette.hint)
            hint = "Enter address"
            setSingleLine(true)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.MIDDLE
            background = null
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_GO
            setPadding(0, 0, 0, 0)
            setSelectAllOnFocus(true)
            setOnEditorActionListener { _, actionId, event ->
                val enterPressed = event?.keyCode == android.view.KeyEvent.KEYCODE_ENTER &&
                    event.action == android.view.KeyEvent.ACTION_UP
                if (actionId == EditorInfo.IME_ACTION_GO || enterPressed) {
                    navigateFromAddressBar(activity)
                    true
                } else {
                    false
                }
            }
            setOnFocusChangeListener { _, hasFocus ->
                if (!hasFocus) {
                    updateAddressBar(activeWebView?.url.orEmpty())
                }
            }
        }
        val refreshBtn = createIconButton(activity, R.drawable.icon_browser_refresh, 32, palette.secondaryIcon) {
            activeWebView?.reload()
            scheduleChromeRefreshSoon()
        }

        addressPill.addView(addrIcon)
        addressPill.addView(addrField)
        addressPill.addView(refreshBtn)
        addressRow.addView(addressPill)
        bar.addView(addressRow)

        val actionRow = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        val backBtn = createIconButton(activity, R.drawable.icon_back, 32, palette.secondaryIcon) {
            navigateBack()
        }
        val fwdBtn = createIconButton(activity, R.drawable.icon_forward, 32, palette.secondaryIcon) {
            navigateForward()
        }
        val plusBtn = createIconButton(activity, R.drawable.icon_plus, 34, palette.icon) {
            openNewTab(activity)
        }
        val tabsBtn = createTabsButton(activity) {
            showTabSwitcher(activity)
        }
        val menuBtn = createIconButton(activity, R.drawable.icon_menu, 34, palette.icon) { anchor ->
            showOverflowMenu(activity, anchor)
        }
        val closeBtn = createIconButton(activity, R.drawable.icon_close_x, 34, palette.icon) {
            dismiss()
        }

        val asideRefreshBtn = createIconButton(activity, R.drawable.icon_browser_refresh, 32, palette.icon) {
            activeWebView?.reload()
            scheduleChromeRefreshSoon()
        }
        asideRefreshBtn.visibility = View.GONE

        actionRow.addView(backBtn)
        actionRow.addView(fwdBtn)
        actionRow.addView(asideRefreshBtn)
        actionRow.addView(View(activity), LinearLayout.LayoutParams(0, 1, 1f))
        actionRow.addView(plusBtn)
        actionRow.addView(tabsBtn)
        actionRow.addView(menuBtn)
        actionRow.addView(closeBtn)
        bar.addView(actionRow)

        addressIcon = addrIcon
        addressField = addrField
        this.addressRow = addressRow
        backButton = backBtn
        forwardButton = fwdBtn
        asideRefreshButton = asideRefreshBtn
        plusButton = plusBtn
        menuButton = menuBtn
        updateNavigationButtons()
        return bar
    }

    private fun beginAttachActiveTab(activity: Activity, initialUrl: String = "") {
        val tabId = activeTabId ?: return
        pendingAttachToken += 1
        val token = pendingAttachToken
        pendingTabId = tabId
        attachActiveTab(activity, tabId, initialUrl, 0, token)
    }

    private fun attachActiveTab(
        activity: Activity,
        tabId: String,
        initialUrl: String,
        attempt: Int,
        token: Long
    ) {
        if (pendingAttachToken != token || pendingTabId != tabId || activeTabId != tabId) {
            return
        }

        val managedWebView = findManagedWebView(tabId)
        if (managedWebView == null) {
            if (attempt >= ATTACH_MAX_RETRIES) {
                pendingTabId = null
                Log.w(TAG, "show failed: managed WebView not found for tabId=$tabId")
                closeTab(tabId)
                return
            }
            activity.window.decorView.postDelayed(
                { attachActiveTab(activity, tabId, initialUrl, attempt + 1, token) },
                ATTACH_RETRY_DELAY_MS
            )
            return
        }

        pendingTabId = null
        attachWebView(managedWebView, tabId, initialUrl)
    }

    private fun attachWebView(managedWebView: LingXiaWebView, tabId: String, initialUrl: String) {
        val host = contentHost ?: return
        if (activeWebView !== managedWebView) {
            activeWebView?.pause()
            activeWebView?.let { previous ->
                (previous.parent as? ViewGroup)?.removeView(previous)
            }
        }
        (managedWebView.parent as? ViewGroup)?.removeView(managedWebView)
        managedWebView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT
        )
        managedWebView.visibility = View.VISIBLE
        host.removeAllViews()
        host.addView(managedWebView)
        managedWebView.resume()

        managedWebView.setOnTouchListener { _, event ->
            if (event.action == android.view.MotionEvent.ACTION_DOWN) {
                markActiveTabInteracted()
            }
            false
        }
        activeWebView = managedWebView
        activeWebViewTabId = tabId
        updateAddressBar(initialUrl.ifEmpty { managedWebView.url.orEmpty() })
        refreshChromeFromActiveWebView()
        scheduleChromeRefreshSoon()
    }

    private fun openNewTab(activity: Activity) {
        if (isAsideActive) {
            return
        }
        closeOverflowMenu()
        closeTabSwitcher()
        val appId = LxApp.homeAppId?.takeIf { it.isNotBlank() }
        if (appId == null) {
            Log.w(TAG, "openNewTab failed: no home appId")
            return
        }
        val sessionId = NativeApi.getLxAppSessionId(appId)
        if (sessionId <= 0L) {
            Log.w(TAG, "openNewTab failed: invalid session for appId=$appId")
            return
        }
        val tabId = NativeApi.openTrustedBrowserTabWithId(appId, sessionId, HIDDEN_NEW_TAB_URL, "tab-" + java.util.UUID.randomUUID().toString())
        if (tabId.isNullOrBlank()) {
            Log.w(TAG, "openNewTab failed: native open returned empty tab")
            return
        }
        show(activity, tabId, HIDDEN_NEW_TAB_URL)
    }

    private fun activateTab(activity: Activity, tabId: String) {
        val normalizedTabId = normalizeTabId(tabId)
        if (!openTabIds.contains(normalizedTabId) || tabIsAside(normalizedTabId) != isAsideActive) {
            return
        }
        activeTabId = normalizedTabId
        NativeApi.browserTabActivate(normalizedTabId)
        onActiveTabSwitched(activity, normalizedTabId)
        closeOverflowMenu()
        closeTabSwitcher()
        beginAttachActiveTab(activity)
    }

    private fun closeTab(tabId: String) {
        val normalizedTabId = normalizeTabId(tabId)
        val closingAside = tabIsAside(normalizedTabId)
        val groupIndex = tabIdsForMode(closingAside).indexOf(normalizedTabId)
        val index = openTabIds.indexOf(normalizedTabId)
        if (index < 0) {
            closeBrowserTab(normalizedTabId)
            return
        }

        if (activeWebViewTabId == normalizedTabId) {
            activeWebView?.pause()
            activeWebView?.let { view ->
                (view.parent as? ViewGroup)?.removeView(view)
            }
            activeWebView = null
            activeWebViewTabId = null
        }
        openTabIds.removeAt(index)
        interactedTabIds.remove(normalizedTabId)
        closeBrowserTab(normalizedTabId)

        if (activeTabId == normalizedTabId) {
            val remaining = tabIdsForMode(closingAside)
            if (remaining.isEmpty()) {
                activeTabId = null
                dismiss()
                return
            }
            val nextIndex = groupIndex.coerceAtLeast(0).coerceAtMost(remaining.lastIndex)
            currentActivity?.let { activity ->
                activeTabId = remaining[nextIndex]
                NativeApi.browserTabActivate(activeTabId!!)
                onActiveTabSwitched(activity, activeTabId!!)
                beginAttachActiveTab(activity)
            }
        }
        updateTabsBadge()
    }

    private fun navigateFromAddressBar(activity: Activity) {
        if (isAsideActive) {
            updateAddressBar(activeWebView?.url.orEmpty())
            return
        }
        val raw = addressField?.text?.toString().orEmpty()
        hideKeyboard(activity, addressField)
        addressField?.clearFocus()
        val targetUrl = normalizeAddressInput(raw)
        if (targetUrl == null) {
            updateAddressBar(activeWebView?.url.orEmpty())
            return
        }
        // An address-bar navigation is a user interaction.
        markActiveTabInteracted()
        navigateActiveTab(activity, targetUrl)
    }

    private fun showTabSwitcher(activity: Activity) {
        val container = overlayContainer ?: return
        closeOverflowMenu()
        closeTabSwitcher()

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(palette.scrim)
            isClickable = true
            setOnClickListener { closeTabSwitcher() }
        }
        // Edge-to-edge bottom sheet: flush with the screen sides/bottom, only
        // the top corners rounded, content padded past the navigation bar.
        val navInset = ViewCompat.getRootWindowInsets(container)
            ?.getInsets(WindowInsetsCompat.Type.systemBars())?.bottom ?: 0
        val panel = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM
            }
            background = GradientDrawable().apply {
                setColor(palette.surface)
                cornerRadii = floatArrayOf(
                    dp(activity, 16).toFloat(), dp(activity, 16).toFloat(),
                    dp(activity, 16).toFloat(), dp(activity, 16).toFloat(),
                    0f, 0f,
                    0f, 0f
                )
            }
            elevation = dp(activity, 12).toFloat()
            setPadding(dp(activity, 12), dp(activity, 10), dp(activity, 12), dp(activity, 12) + navInset)
            setOnClickListener { }
        }

        val header = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 44)
            )
        }
        header.addView(TextView(activity).apply {
            text = "Tabs"
            setTextColor(palette.title)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 17f)
            setTypeface(typeface, android.graphics.Typeface.BOLD)
            layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
        })
        // New tabs are self mode; hide the affordance while an aside is active.
        if (!isAsideActive) {
            header.addView(createIconButton(activity, R.drawable.icon_plus, 34, palette.icon) {
                closeTabSwitcher()
                openNewTab(activity)
            })
        }
        panel.addView(header)

        val list = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }
        val visibleTabs = tabIdsForMode()
        visibleTabs.forEach { tabId ->
            list.addView(createTabRow(activity, tabId))
        }
        // Size to the rows (52dp each) and only cap when the list is long, so
        // one tab doesn't float in a half-screen sheet.
        val contentHeight = dp(activity, 52) * visibleTabs.size.coerceAtLeast(1)
        val maxHeight = minOf(dp(activity, 360), activity.resources.displayMetrics.heightPixels / 2)
        panel.addView(ScrollView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                minOf(contentHeight, maxHeight)
            )
            addView(list)
        })

        overlay.addView(panel)
        container.addView(overlay)
        tabSwitcher = overlay
    }

    private fun createTabRow(activity: Activity, tabId: String): View {
        val isActive = tabId == activeTabId
        return LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 52)
            )
            background = GradientDrawable().apply {
                setColor(if (isActive) palette.activeRow else Color.TRANSPARENT)
                cornerRadius = dp(activity, 8).toFloat()
            }
            setPadding(dp(activity, 10), 0, dp(activity, 4), 0)
            isClickable = true
            setOnClickListener { activateTab(activity, tabId) }

            addView(TextView(activity).apply {
                text = tabTitle(tabId)
                setTextColor(palette.title)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                setSingleLine(true)
                ellipsize = TextUtils.TruncateAt.END
                layoutParams = LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f)
            })
            addView(createIconButton(activity, R.drawable.icon_close_x, 32, palette.secondaryIcon) {
                closeTab(tabId)
                if (isShowing()) {
                    showTabSwitcher(activity)
                }
            })
        }
    }

    private fun closeTabSwitcher() {
        tabSwitcher?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        tabSwitcher = null
    }

    private fun closeOverflowMenu() {
        overflowMenu?.let { view ->
            (view.parent as? ViewGroup)?.removeView(view)
        }
        overflowMenu = null
    }

    private fun showOverflowMenu(activity: Activity, anchor: View) {
        val container = overlayContainer ?: return
        if (overflowMenu != null) {
            closeOverflowMenu()
            return
        }
        closeTabSwitcher()

        val overlay = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.TRANSPARENT)
            elevation = dp(activity, 32).toFloat()
            translationZ = dp(activity, 32).toFloat()
            isClickable = true
            setOnClickListener { closeOverflowMenu() }
        }

        val panelWidth = dp(activity, 188)
        val containerLocation = IntArray(2)
        val anchorLocation = IntArray(2)
        container.getLocationInWindow(containerLocation)
        anchor.getLocationInWindow(anchorLocation)
        val containerWidth = container.width.takeIf { it > 0 } ?: activity.resources.displayMetrics.widthPixels
        val containerHeight = container.height.takeIf { it > 0 } ?: activity.resources.displayMetrics.heightPixels
        val anchorRight = anchorLocation[0] - containerLocation[0] + anchor.width
        val rightMargin = (containerWidth - anchorRight).coerceAtLeast(dp(activity, 12))
        val barTop = bottomBar?.let { bar ->
            val location = IntArray(2)
            bar.getLocationInWindow(location)
            location[1] - containerLocation[1]
        }?.takeIf { it > 0 }
        val bottomMargin = if (barTop != null) {
            (containerHeight - barTop + dp(activity, 8)).coerceAtLeast(dp(activity, 12))
        } else {
            (bottomBar?.height ?: dp(activity, 96)) + getNavigationBarHeight(activity) + dp(activity, 10)
        }

        val panel = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = FrameLayout.LayoutParams(
                panelWidth,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                gravity = Gravity.BOTTOM or Gravity.END
                this.rightMargin = rightMargin
                this.bottomMargin = bottomMargin
            }
            background = GradientDrawable().apply {
                setColor(palette.surface)
                cornerRadius = dp(activity, 12).toFloat()
                setStroke(dp(activity, 1), palette.stroke)
            }
            elevation = dp(activity, 14).toFloat()
            setPadding(dp(activity, 6), dp(activity, 6), dp(activity, 6), dp(activity, 6))
            setOnClickListener { }
        }

        panel.addView(
            createOverflowMenuRow(
                activity,
                R.drawable.icon_browser_download,
                Lingxia.localizedString(activity, R.string.lx_browser_downloads)
            ) {
                closeOverflowMenu()
                navigateActiveTab(activity, "lingxia://downloads")
            }
        )
        panel.addView(
            createOverflowMenuRow(
                activity,
                R.drawable.icon_browser_settings,
                Lingxia.localizedString(activity, R.string.lx_browser_settings)
            ) {
                closeOverflowMenu()
                navigateActiveTab(activity, "lingxia://settings")
            }
        )

        overlay.addView(panel)
        container.addView(overlay)
        overlay.bringToFront()
        overflowMenu = overlay
    }

    private fun createOverflowMenuRow(
        activity: Activity,
        resId: Int,
        title: String,
        onClick: () -> Unit
    ): View {
        return LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(activity, 48)
            )
            background = GradientDrawable().apply {
                setColor(Color.TRANSPARENT)
                cornerRadius = dp(activity, 8).toFloat()
            }
            setPadding(dp(activity, 12), 0, dp(activity, 12), 0)
            isClickable = true
            isFocusable = true
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackground, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            setOnClickListener { onClick() }

            addView(ImageView(activity).apply {
                layoutParams = LinearLayout.LayoutParams(dp(activity, 22), dp(activity, 22)).apply {
                    rightMargin = dp(activity, 12)
                }
                scaleType = ImageView.ScaleType.CENTER_INSIDE
                setImageResource(resId)
                setColorFilter(palette.icon)
            })
            addView(TextView(activity).apply {
                text = title
                setTextColor(palette.title)
                setTextSize(TypedValue.COMPLEX_UNIT_SP, 15f)
                includeFontPadding = false
                layoutParams = LinearLayout.LayoutParams(
                    0,
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    1f
                )
            })
        }
    }

    private fun updateAddressBar(url: String) {
        val cleanUrl = url.trim()
        val hidden = cleanUrl.isEmpty() || cleanUrl.equals(HIDDEN_NEW_TAB_URL, ignoreCase = true)
        val field = addressField
        if (field != null && !field.hasFocus()) {
            field.setText(if (hidden) "" else displayUrl(cleanUrl))
        }

        val icon = addressIcon ?: return
        if (hidden) {
            icon.visibility = View.GONE
            return
        }
        // Secure is the norm — no padlock (misread as "locked"); only insecure
        // pages get an icon.
        val scheme = runCatching { URI(cleanUrl).scheme?.lowercase() }.getOrNull()
        if (scheme == "https" || scheme == "lingxia") {
            icon.visibility = View.GONE
        } else {
            icon.visibility = View.VISIBLE
            icon.setImageResource(R.drawable.icon_warning)
            icon.setColorFilter(palette.warning)
        }
    }

    private fun updateNavigationButtons() {
        val view = activeWebView
        // Pre-interaction history is auto-created (redirects/pushState) and
        // must not light the affordances.
        val interacted = activeTabId?.let(interactedTabIds::contains) == true
        setButtonEnabled(backButton, view?.canGoBack() == true && interacted)
        setButtonEnabled(forwardButton, view?.canGoForward() == true && interacted)
        updateTabsBadge()
    }

    private fun updateTabsBadge() {
        val count = tabIdsForMode().size.coerceAtLeast(1)
        tabsBadge?.text = if (count > 99) "99+" else count.toString()
    }

    private fun refreshChromeFromActiveWebView() {
        // During attach retry the displayed webview still belongs to the
        // previous tab; address and back/forward are per-tab, keep them reset.
        if (activeWebViewTabId != activeTabId) {
            updateTabsBadge()
            return
        }
        updateAddressBar(activeWebView?.url.orEmpty())
        updateNavigationButtons()
    }

    // Blank the per-tab chrome immediately on a tab switch and re-derive the
    // aside styling for the new tab.
    private fun onActiveTabSwitched(activity: Activity, tabId: String) {
        addressField?.setText("")
        setButtonEnabled(backButton, false)
        setButtonEnabled(forwardButton, false)
        isAsideActive = NativeApi.browserTabIsAside(tabId)
        applyActiveModeChrome(activity)
    }

    private fun currentBarHeightDp(): Int = if (isAsideActive) 52 else 96

    // Compact aside chrome is intentionally one row. The desktop docked aside
    // may expose a read-only address, but that projection must not leak here.
    private fun applyActiveModeChrome(activity: Activity) {
        val aside = isAsideActive
        if (aside && addressField?.hasFocus() == true) {
            hideKeyboard(activity, addressField)
            addressField?.clearFocus()
        }
        addressRow?.visibility = if (aside) View.GONE else View.VISIBLE
        plusButton?.visibility = if (aside) View.GONE else View.VISIBLE
        menuButton?.visibility = if (aside) View.GONE else View.VISIBLE
        asideRefreshButton?.visibility = if (aside) View.VISIBLE else View.GONE
        updateTabsBadge()
        overlayContainer?.let(ViewCompat::requestApplyInsets)
    }

    private fun markActiveTabInteracted() {
        val tabId = activeTabId ?: return
        if (!interactedTabIds.add(tabId)) {
            return
        }
        updateNavigationButtons()
    }

    private fun startChromeRefreshLoop() {
        val container = overlayContainer ?: return
        container.removeCallbacks(chromeRefreshRunnable)
        container.postDelayed(chromeRefreshRunnable, 400L)
    }

    private fun scheduleChromeRefresh() {
        overlayContainer?.postDelayed(chromeRefreshRunnable, 400L)
    }

    private fun scheduleChromeRefreshSoon() {
        val container = overlayContainer ?: return
        container.removeCallbacks(chromeRefreshRunnable)
        container.postDelayed(chromeRefreshRunnable, 120L)
    }

    private fun stopChromeRefreshLoop() {
        overlayContainer?.removeCallbacks(chromeRefreshRunnable)
    }

    private fun navigateActiveTab(activity: Activity, targetUrl: String): Boolean {
        val tabId = activeTabId ?: return false
        updateAddressBar(targetUrl)
        val navigated = if (targetUrl == "lingxia://downloads" || targetUrl == "lingxia://settings") {
            val appId = LxApp.homeAppId ?: return false
            val sessionId = NativeApi.getLxAppSessionId(appId)
            sessionId > 0L && NativeApi.openTrustedBrowserTabWithId(appId, sessionId, targetUrl, tabId) != null
        } else {
            NativeApi.browserTabNavigate(tabId, targetUrl)
        }
        if (!navigated) {
            Log.w(TAG, "navigate failed: tabId=$tabId url=$targetUrl")
            scheduleChromeRefreshSoon()
            return false
        }
        beginAttachActiveTab(activity, targetUrl)
        scheduleChromeRefreshSoon()
        return true
    }

    private fun navigateBack() {
        val view = activeWebView ?: return
        if (view.canGoBack()) {
            view.goBack()
            scheduleChromeRefreshSoon()
        } else {
            updateNavigationButtons()
        }
    }

    private fun navigateForward() {
        val view = activeWebView ?: return
        if (view.canGoForward()) {
            view.goForward()
            scheduleChromeRefreshSoon()
        } else {
            updateNavigationButtons()
        }
    }

    private fun registerTab(tabId: String) {
        if (!openTabIds.contains(tabId)) {
            openTabIds.add(tabId)
        }
    }

    private fun tabIsAside(tabId: String): Boolean =
        runCatching { NativeApi.browserTabIsAside(tabId) }.getOrDefault(false)

    private fun tabIdsForMode(aside: Boolean = isAsideActive): List<String> =
        openTabIds.filter { tabIsAside(it) == aside }

    private fun findManagedWebView(tabId: String): LingXiaWebView? =
        NativeApi.findBrowserTabWebView(tabId)

    private fun closeBrowserTab(tabId: String) {
        if (tabId.isBlank()) return
        NativeApi.browserTabClose(tabId)
    }

    private fun tabTitle(tabId: String): String {
        val view = findManagedWebView(tabId)
        val title = view?.title?.trim()
        if (!title.isNullOrEmpty()) {
            return title
        }
        val url = view?.url?.trim().orEmpty()
        if (url.isEmpty() || url.equals(HIDDEN_NEW_TAB_URL, ignoreCase = true)) {
            return "New Tab"
        }
        return runCatching {
            URI(url).host?.removePrefix("www.")?.takeIf { it.isNotBlank() }
        }.getOrNull() ?: url
    }

    private fun displayUrl(url: String): String {
        return runCatching {
            val uri = URI(url)
            val host = uri.host?.removePrefix("www.")
            if (!host.isNullOrBlank() && uri.scheme in setOf("http", "https")) {
                val path = uri.rawPath?.takeIf { it.isNotBlank() && it != "/" }.orEmpty()
                val query = uri.rawQuery?.let { "?$it" }.orEmpty()
                "$host$path$query"
            } else {
                url
            }
        }.getOrDefault(url)
    }

    private fun normalizeAddressInput(raw: String): String? {
        val input = raw.trim()
        if (input.isEmpty()) {
            return HIDDEN_NEW_TAB_URL
        }
        val explicitScheme = runCatching { URI(input).scheme?.lowercase() }.getOrNull()
        if (explicitScheme == "http" || explicitScheme == "https" || explicitScheme == "lingxia") {
            return input
        }
        val looksLikeHost = input.contains(".") && !input.contains(" ")
        return if (looksLikeHost) "https://$input" else null
    }

    private fun createTabsButton(activity: Activity, onClick: (View) -> Unit): View {
        val frame = FrameLayout(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, 38), dp(activity, 38)).apply {
                leftMargin = dp(activity, 2)
                rightMargin = dp(activity, 2)
            }
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick(it) }
        }
        frame.addView(ImageView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(dp(activity, 24), dp(activity, 24), Gravity.CENTER)
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setImageResource(R.drawable.icon_tabs)
            setColorFilter(palette.icon)
        })
        val badge = TextView(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.CENTER
            )
            background = null
            gravity = Gravity.CENTER
            setPadding(0, 0, 0, 0)
            setTextColor(palette.icon)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 10f)
            setTypeface(typeface, android.graphics.Typeface.BOLD)
            includeFontPadding = false
            translationX = dp(activity, 2).toFloat()
            translationY = -dp(activity, 2).toFloat()
        }
        frame.addView(badge)
        tabsBadge = badge
        updateTabsBadge()
        return frame
    }

    private fun createIconButton(
        activity: Activity,
        resId: Int,
        sizeDp: Int = 34,
        tint: Int = palette.icon,
        onClick: (View) -> Unit
    ): ImageView {
        return ImageView(activity).apply {
            layoutParams = LinearLayout.LayoutParams(dp(activity, sizeDp), dp(activity, sizeDp)).apply {
                leftMargin = dp(activity, 2)
                rightMargin = dp(activity, 2)
            }
            scaleType = ImageView.ScaleType.CENTER_INSIDE
            setPadding(dp(activity, 6), dp(activity, 6), dp(activity, 6), dp(activity, 6))
            setImageResource(resId)
            setColorFilter(tint)
            val outValue = TypedValue()
            activity.theme.resolveAttribute(
                android.R.attr.selectableItemBackgroundBorderless, outValue, true
            )
            setBackgroundResource(outValue.resourceId)
            isClickable = true
            isFocusable = true
            setOnClickListener { onClick(it) }
        }
    }

    private fun setButtonEnabled(button: ImageView?, enabled: Boolean) {
        button?.isEnabled = enabled
        button?.alpha = if (enabled) 1f else 0.3f
    }

    private fun normalizeTabId(tabId: String): String = tabId.trim()

    private fun hideKeyboard(activity: Activity, target: View?) {
        val inputMethod = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as? InputMethodManager
        inputMethod?.hideSoftInputFromWindow(target?.windowToken, 0)
    }

    private fun dp(activity: Activity, value: Int): Int {
        return (value * activity.resources.displayMetrics.density + 0.5f).toInt()
    }

    private fun getNavigationBarHeight(activity: Activity): Int {
        val resources = activity.resources
        val resourceId = resources.getIdentifier("navigation_bar_height", "dimen", "android")
        return if (resourceId > 0) resources.getDimensionPixelSize(resourceId) else 0
    }
}
