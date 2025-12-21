package com.lingxia.lxapp

import android.content.Context
import android.graphics.Color
import android.util.AttributeSet
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.ProgressBar
import android.widget.TextView
import androidx.annotation.AttrRes
import androidx.annotation.StyleRes


/**
 * Configuration data class for the NavigationBar
 * Updated to use new Rust API with boolean fields
 */
data class NavigationBarState(
    val navigationBarBackgroundColor: Int,         // Background color (e.g., #FFFFFF)
    val navigationBarTextStyle: String,            // Text style ("black" or "white")
    val navigationBarTitleText: String,            // Navigation bar title text
    val showNavbar: Boolean,                       // Whether to show the navigation bar
    val showBackButton: Boolean,                   // Whether to show the back button
    val showHomeButton: Boolean                    // Whether to show the home button
) {
    companion object {
        // Default values
        val DEFAULT_BACKGROUND_COLOR = Color.WHITE
        val DEFAULT_TEXT_COLOR = Color.BLACK
        const val DEFAULT_HEIGHT_DP = LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP
    }
}

/**
 * Custom Navigation Bar view mimicking WeChat Mini Program behavior.
 */
class NavigationBar @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    @AttrRes defStyleAttr: Int = 0,
    @StyleRes defStyleRes: Int = 0
) : FrameLayout(context, attrs, defStyleAttr, defStyleRes) {

    companion object {
        private const val TAG = "LingXia.NavigationBar"
        // Default colors - internal access
        internal val DEFAULT_BACKGROUND_COLOR = Color.WHITE
        internal val DEFAULT_FRONT_COLOR = Color.BLACK // Default text/icon color

        // Define a specific height for tablets - should be same as phone for now
        private const val DEFAULT_TABLET_HEIGHT_DP = 44 // Same as LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP
    }

    /**
     * Color utility functions for NavigationBar
     */
    object ColorUtils {
        /**
         * Helper function to determine if a color is dark
         */
        fun isColorDark(argbColor: Int): Boolean {
            // Extract RGB components from ARGB
            val red = (argbColor shr 16) and 0xFF
            val green = (argbColor shr 8) and 0xFF
            val blue = argbColor and 0xFF

            // Calculate luminance using standard formula
            val luminance = 0.299 * red + 0.587 * green + 0.114 * blue

            // Consider colors with luminance < 128 as dark (0-255 scale)
            return luminance < 128
        }

        /**
         * Resolve front text/icon color based on navbar text style and background color
         */
        fun resolveNavTextColor(navbarState: NavigationBarState): Int {
            return when (navbarState.navigationBarTextStyle.lowercase()) {
                "white" -> Color.WHITE
                "black" -> Color.BLACK
                else -> if (isColorDark(navbarState.navigationBarBackgroundColor)) Color.WHITE else Color.BLACK
            }
        }
    }

    private val titleTextView: TextView
    private val loadingIndicator: ProgressBar
    private val backButton: ImageView
    private val homeButton: ImageView
    private var currentConfig: NavigationBarState = NavigationBarState(
        navigationBarBackgroundColor = Color.WHITE,
        navigationBarTextStyle = "black",
        navigationBarTitleText = "",
        showNavbar = true,
        showBackButton = false,
        showHomeButton = false
    )
    private var knownStatusBarHeight: Int = 0

    // Store current colors
    private var currentBackgroundColor = DEFAULT_BACKGROUND_COLOR
    private var currentFrontColor = DEFAULT_FRONT_COLOR

    // Callbacks
    private var onBackClickListener: (() -> Unit)? = null

    private var homeButtonDrawable: LxAppDrawables.HomeButtonDrawable? = null

    init {
        val density = resources.displayMetrics.density

        // Determine if it's a tablet (smallest width >= 600dp)
        val smallestScreenWidthDp = context.resources.configuration.smallestScreenWidthDp
        val isTablet = smallestScreenWidthDp >= 600

        val navBarHeightDp = if (isTablet) DEFAULT_TABLET_HEIGHT_DP else LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP

        val heightPx = (navBarHeightDp * density).toInt()
        val buttonSizePx = (LxAppDrawables.Constants.BUTTON_SIZE_DP * density).toInt()
        val topMarginPx = (42 * density).toInt()

        setBackgroundColor(currentBackgroundColor)

        homeButtonDrawable = LxAppDrawables.createHomeButton(resources, currentFrontColor)

        backButton = ImageView(context).apply {
            layoutParams = LayoutParams(buttonSizePx, buttonSizePx).apply {
                gravity = Gravity.START or Gravity.CENTER_VERTICAL
                marginStart = (4 * density).toInt()
            }
            contentDescription = "Back"
            visibility = View.GONE
        }
        LxAppDrawables.configureBackButton(backButton)
        addView(backButton)

        // Home Button setup (same position as back button since only one shows at a time)
        homeButton = ImageView(context).apply {
            layoutParams = LayoutParams(buttonSizePx, buttonSizePx).apply {
                gravity = Gravity.START or Gravity.CENTER_VERTICAL
                marginStart = (8 * density).toInt()
            }
            setBackgroundColor(Color.TRANSPARENT)
            setImageDrawable(homeButtonDrawable)
            contentDescription = "Home"
            visibility = View.GONE
        }
        addView(homeButton)

        // Calculate dynamic font size for title
        val targetTitleSp = if (isTablet) 12f else 17f

        // Title TextView setup
        titleTextView = TextView(context).apply {
            layoutParams = LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.CENTER_HORIZONTAL or Gravity.CENTER_VERTICAL
                // Remove topMargin to use CENTER_VERTICAL alignment like buttons
            }
            gravity = Gravity.CENTER
            textAlignment = View.TEXT_ALIGNMENT_CENTER
            setTextColor(currentFrontColor)
            includeFontPadding = false
            setTextSize(TypedValue.COMPLEX_UNIT_SP, targetTitleSp)
            typeface = android.graphics.Typeface.create("sans-serif-medium", android.graphics.Typeface.NORMAL)
            visibility = View.VISIBLE
        }
        addView(titleTextView)

        // Loading Indicator setup
        val progressBarSize = (24 * density).toInt()
        loadingIndicator = ProgressBar(context, null, android.R.attr.progressBarStyleSmall).apply {
            layoutParams = LayoutParams(progressBarSize, progressBarSize).apply {
                gravity = Gravity.CENTER_VERTICAL or Gravity.START
                marginStart = (16 * density).toInt()
            }
            updateProgressColor(currentFrontColor)
            visibility = View.GONE
        }
        addView(loadingIndicator)
    }

    // Helper method to update progress indicator color
    private fun ProgressBar.updateProgressColor(color: Int) {
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
            indeterminateDrawable?.colorFilter = android.graphics.BlendModeColorFilter(color, android.graphics.BlendMode.SRC_IN)
        } else {
            @Suppress("DEPRECATION")
            indeterminateDrawable?.setColorFilter(color, android.graphics.PorterDuff.Mode.SRC_IN)
        }
    }

    /**
     * Returns the calculated content height in pixels based on device type (phone/tablet).
     */
    fun getCalculatedContentHeightPx(): Int {
        val density = resources.displayMetrics.density
        val isTablet = context.resources.configuration.smallestScreenWidthDp >= 600
        val navBarHeightDp = if (isTablet) DEFAULT_TABLET_HEIGHT_DP else LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP
        return (navBarHeightDp * density).toInt()
    }

    /**
     * Shows the loading indicator in the navigation bar.
     */
    fun showLoading() {
        loadingIndicator.visibility = View.VISIBLE
    }

    /**
     * Hides the loading indicator in the navigation bar.
     */
    fun hideLoading() {
        loadingIndicator.visibility = View.GONE
    }

    /**
     * Sets the title text displayed in the navigation bar.
     *
     * @param title The text to display.
     */
    fun setTitle(title: String?) {
        titleTextView.text = title ?: ""
    }

    /**
     * Sets the background color and front color (for title and loading indicator) of the navigation bar.
     *
     * @param backgroundColor The background color (e.g., Color.parseColor("#ffffff")).
     * @param frontColor The color for text and icons (e.g., Color.parseColor("#000000")).
     */
    fun setColor(backgroundColor: Int, frontColor: Int) {
        currentBackgroundColor = backgroundColor
        currentFrontColor = frontColor

        setBackgroundColor(currentBackgroundColor)
        titleTextView.setTextColor(currentFrontColor)

        // Update loading indicator color
        loadingIndicator.updateProgressColor(currentFrontColor)

        // Update home button drawable color
        homeButtonDrawable?.updateColor(currentFrontColor)
    }

    /**
     * Sets the visibility of the back button.
     *
     * @param visible Whether the back button should be visible.
     */
    fun setBackButtonVisible(visible: Boolean) {
        backButton.visibility = if (visible) View.VISIBLE else View.GONE
    }

    /**
     * Sets the visibility of the home button.
     *
     * @param visible Whether the home button should be visible.
     */
    fun setHomeButtonVisible(visible: Boolean) {
        homeButton.visibility = if (visible) View.VISIBLE else View.GONE

        // Force redraw if needed
        if (width > 0 && height > 0) {
            homeButton.invalidate()
        } else {
            post { homeButton.invalidate() }
        }
    }

    /**
     * Sets a listener for back button clicks
     *
     * @param listener The callback to invoke when the back button is clicked
     */
    fun setOnBackButtonClickListener(listener: OnClickListener) {
        backButton.setOnClickListener(listener)
    }

    /**
     * Sets a listener for home button clicks
     *
     * @param listener The callback to invoke when the home button is clicked
     */
    fun setOnHomeButtonClickListener(listener: OnClickListener) {
        homeButton.setOnClickListener(listener)
    }

    /**
     * Shows the navigation bar.
     */
    fun show() {
        visibility = View.VISIBLE
    }

    /**
     * Hides the navigation bar.
     */
    fun hide() {
        visibility = View.GONE
    }

    /**
     * Updates the state of the NavigationBar and optionally animates the transition.
     *
     * @param title The title text to display.
     * @param bgColor The background color.
     * @param textColor The text color (for title and button icons).
     * @param showBackButton Whether the back button should be initially visible.
     * @param showHomeButton Whether the home button should be initially visible.
     * @param isBackNavigation Direction hint for animation.
     * @param disableAnimation If true, update instantly; otherwise, animate.
     * @param onBackClickListener Listener for the back button.
     * @param onHomeClickListener Listener for the home button.
     * @param onAnimationEnd Optional Runnable to execute after animation finishes.
     */
    fun updateStateAndAnimate(
        title: String,
        bgColor: Int,
        textColor: Int,
        showBackButton: Boolean,
        showHomeButton: Boolean = false,
        isBackNavigation: Boolean,
        disableAnimation: Boolean,
        onBackClickListener: OnClickListener,
        onHomeClickListener: OnClickListener? = null,
        onAnimationEnd: Runnable? = null
    ) {
        // Set state
        setTitle(title)
        setColor(bgColor, textColor)

        // Only show one button at a time (back button takes priority)
        if (showBackButton) {
            setBackButtonVisible(true)
            setHomeButtonVisible(false)
        } else if (showHomeButton) {
            setBackButtonVisible(false)
            setHomeButtonVisible(true)
        } else {
            setBackButtonVisible(false)
            setHomeButtonVisible(false)
        }

        setOnBackButtonClickListener(onBackClickListener)
        onHomeClickListener?.let { setOnHomeButtonClickListener(it) }

        if (!disableAnimation) {
            animate()
                .translationX(0f)
                .setDuration(LxAppDrawables.Constants.ANIMATION_DURATION_MS)
                .setInterpolator(android.view.animation.DecelerateInterpolator())
                .withEndAction {
                    translationX = 0f
                    onAnimationEnd?.run()
                }
                .start()
        } else {
            translationX = 0f
            onAnimationEnd?.run()
        }
    }

    /**
     * Configures the NavigationBar based on NavigationBarState.
     * Handles both full navbar display and button-only mode when navbar is hidden.
     */
    fun configure(
        navbarState: NavigationBarState,
        onBackClickListener: OnClickListener,
        onHomeClickListener: OnClickListener? = null,
        disableAnimation: Boolean = false
    ) {
        val textColor = ColorUtils.resolveNavTextColor(navbarState)

        if (navbarState.showNavbar) {
            visibility = View.VISIBLE
            // Show full navbar
            updateStateAndAnimate(
                title = navbarState.navigationBarTitleText,
                bgColor = navbarState.navigationBarBackgroundColor,
                textColor = textColor,
                showBackButton = navbarState.showBackButton,
                showHomeButton = navbarState.showHomeButton,
                isBackNavigation = false,
                disableAnimation = disableAnimation,
                onBackClickListener = onBackClickListener,
                onHomeClickListener = onHomeClickListener
            )
        } else if (navbarState.showBackButton || navbarState.showHomeButton) {
            visibility = View.VISIBLE
            // Show button-only mode (transparent background, no title)
            updateStateAndAnimate(
                title = "",
                bgColor = Color.TRANSPARENT,
                textColor = textColor,
                showBackButton = navbarState.showBackButton,
                showHomeButton = navbarState.showHomeButton,
                isBackNavigation = false,
                disableAnimation = disableAnimation,
                onBackClickListener = onBackClickListener,
                onHomeClickListener = onHomeClickListener
            )
        } else {
            // Hide completely
            hide()
        }
    }

    // Method to receive status bar height
    fun setExternalStatusBarHeight(sbh: Int) {
        if (knownStatusBarHeight != sbh) {
            knownStatusBarHeight = sbh

            val density = resources.displayMetrics.density
            val baseTopMargin = (8 * density).toInt() // Updated offset from init block

            // Update layout params of children that depend on status bar height
            // Note: buttons use CENTER_VERTICAL so they don't need topMargin adjustment
            // Note: title also uses CENTER_VERTICAL so it doesn't need topMargin adjustment
            // Only update if there are views that actually use topMargin positioning
            listOf(loadingIndicator).forEach { view ->
                (view.layoutParams as? FrameLayout.LayoutParams)?.let {
                    it.topMargin = knownStatusBarHeight + baseTopMargin
                    view.layoutParams = it
                }
            }

            requestLayout() // Request re-layout
        }
    }
}
