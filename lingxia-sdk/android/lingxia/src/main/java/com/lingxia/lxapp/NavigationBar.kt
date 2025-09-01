package com.lingxia.lxapp

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.drawable.Drawable
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
import org.json.JSONObject
import android.view.animation.AccelerateDecelerateInterpolator
import kotlin.math.max
import kotlin.math.min

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

        // Define a specific height for tablets
        private const val DEFAULT_TABLET_HEIGHT_DP = 12
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

    /**
     * Custom back button drawable that draws a chevron "<" shape
     */
    private inner class BackButtonDrawable : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentFrontColor
            style = Paint.Style.STROKE
            strokeWidth = 1.8f * resources.displayMetrics.density
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }

        override fun draw(canvas: Canvas) {
            val width = bounds.width()
            val height = bounds.height()
            val centerY = height / 2f
            val startX = width * 0.55f
            val endX = width * 0.35f

            // Draw the chevron lines
            canvas.drawLine(startX, centerY - height * 0.15f, endX, centerY, paint)
            canvas.drawLine(endX, centerY, startX, centerY + height * 0.15f, paint)
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT

        fun updateColor(color: Int) {
            paint.color = color
            invalidateSelf()
        }
    }

    /**
     * Custom home button drawable that draws a house shape
     */
    private inner class HomeButtonDrawable : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentFrontColor
            style = Paint.Style.STROKE
            strokeWidth = 1.8f * resources.displayMetrics.density
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }

        override fun draw(canvas: Canvas) {
            val width = bounds.width()
            val height = bounds.height()
            val centerX = width / 2f
            val centerY = height / 2f
            val size = minOf(width, height) * 0.4f

            // Draw house outline
            val left = centerX - size / 2
            val right = centerX + size / 2
            val top = centerY - size / 2
            val bottom = centerY + size / 2
            val roofTop = top - size * 0.3f

            // Draw roof (triangle)
            canvas.drawLine(left, top, centerX, roofTop, paint)
            canvas.drawLine(centerX, roofTop, right, top, paint)

            // Draw house body (rectangle)
            canvas.drawLine(left, top, left, bottom, paint)
            canvas.drawLine(right, top, right, bottom, paint)
            canvas.drawLine(left, bottom, right, bottom, paint)
        }

        override fun setAlpha(alpha: Int) {
            paint.alpha = alpha
        }

        override fun setColorFilter(colorFilter: android.graphics.ColorFilter?) {
            paint.colorFilter = colorFilter
        }

        @Deprecated("Deprecated in Java")
        override fun getOpacity(): Int = android.graphics.PixelFormat.TRANSLUCENT

        fun updateColor(color: Int) {
            paint.color = color
            invalidateSelf()
        }
    }

    init {
        val density = resources.displayMetrics.density

        // Determine if it's a tablet (smallest width >= 600dp)
        val smallestScreenWidthDp = context.resources.configuration.smallestScreenWidthDp
        val isTablet = smallestScreenWidthDp >= 600

        val navBarHeightDp = if (isTablet) DEFAULT_TABLET_HEIGHT_DP else LxAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP

        Log.d(TAG, "smallestScreenWidthDp: $smallestScreenWidthDp, isTablet: $isTablet, navBarHeightDp: $navBarHeightDp")
        val heightPx = (navBarHeightDp * density).toInt()

        setBackgroundColor(currentBackgroundColor)

        // Back Button setup
        backButton = ImageView(context).apply {
            layoutParams = LayoutParams(heightPx, heightPx).apply {
                gravity = Gravity.START or Gravity.TOP
                marginStart = (4 * density).toInt()
                topMargin = (42 * density).toInt() // Same as title alignment
            }
            setImageDrawable(BackButtonDrawable())
            contentDescription = "Back"
            visibility = View.GONE
        }
        addView(backButton)

        // Home Button setup (same position as back button since only one shows at a time)
        homeButton = ImageView(context).apply {
            layoutParams = LayoutParams(heightPx, heightPx).apply {
                gravity = Gravity.START or Gravity.TOP
                marginStart = (4 * density).toInt() // Same position as back button
                topMargin = (42 * density).toInt() // Same as title alignment
            }
            setImageDrawable(HomeButtonDrawable())
            contentDescription = "Home"
            visibility = View.GONE
        }
        addView(homeButton)

        // Calculate dynamic font size for title
        val targetTitleSp = if (isTablet) 12f else 17f

        Log.d(TAG, "Device isTablet: $isTablet, navBarHeightDp: $navBarHeightDp, Setting title font size to: $targetTitleSp sp")

        // Title TextView setup
        titleTextView = TextView(context).apply {
            layoutParams = LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.CENTER_HORIZONTAL or Gravity.TOP
                // Align title with buttons: same topMargin
                topMargin = (42 * density).toInt()
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

    // Keep these layout debugging overrides only in debug builds
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        super.onMeasure(widthMeasureSpec, heightMeasureSpec)
        Log.d(TAG, "onMeasure: widthSpec=${MeasureSpec.toString(widthMeasureSpec)}, heightSpec=${MeasureSpec.toString(heightMeasureSpec)}")
        Log.d(TAG, "onMeasure: measuredWidth=$measuredWidth, measuredHeight=$measuredHeight")
    }

    override fun onLayout(changed: Boolean, left: Int, top: Int, right: Int, bottom: Int) {
        super.onLayout(changed, left, top, right, bottom)
        Log.d(TAG, "onLayout: changed=$changed, left=$left, top=$top, right=$right, bottom=$bottom")
        Log.d(TAG, "onLayout: width=$width, height=$height, measuredWidth=$measuredWidth, measuredHeight=$measuredHeight")
        if (titleTextView.visibility == View.VISIBLE) {
            Log.d(TAG, "onLayout: titleTextView.top=${titleTextView.top}, titleTextView.bottom=${titleTextView.bottom}, titleTextView.height=${titleTextView.height}, titleTextView.measuredHeight=${titleTextView.measuredHeight}")
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

        // Update back button color
        (backButton.drawable as? BackButtonDrawable)?.updateColor(currentFrontColor)

        // Update home button color
        (homeButton.drawable as? HomeButtonDrawable)?.updateColor(currentFrontColor)
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
        visibility = View.VISIBLE

        // Set state
        setTitle(title)
        setColor(bgColor, textColor)
        setBackButtonVisible(showBackButton)
        setHomeButtonVisible(showHomeButton)
        setOnBackButtonClickListener(onBackClickListener)
        onHomeClickListener?.let { setOnHomeButtonClickListener(it) }

        if (!disableAnimation) {
            animate()
                .translationX(0f)
                .setDuration(300L)
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

    // Method to receive status bar height
    fun setExternalStatusBarHeight(sbh: Int) {
        if (knownStatusBarHeight != sbh) {
            Log.d(TAG, "ExternalStatusBarHeight set to: $sbh")
            knownStatusBarHeight = sbh

            // Update layout params of children that depend on status bar height
            listOf(backButton, homeButton, loadingIndicator).forEach { view ->
                (view.layoutParams as? FrameLayout.LayoutParams)?.let {
                    it.topMargin = knownStatusBarHeight
                    view.layoutParams = it
                }
            }

            requestLayout() // Request re-layout
        }
    }
}
