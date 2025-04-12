package com.lingxia.miniapp

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

/**
 * Configuration data class for the NavigationBar
 */
data class NavigationBarConfig(
    val hidden: Boolean = false,                   // Whether the navigation bar is hidden
    val navigationBarBackgroundColor: Int? = null, // Background color (e.g., #FFFFFF)
    val navigationBarTextStyle: String? = null,    // Text style ("black" or "white")
    val navigationBarTitleText: String? = null,    // Navigation bar title text
    val navigationStyle: String? = null            // "default" or "custom"
) {
    companion object {
        // Default values
        val DEFAULT_BACKGROUND_COLOR = Color.WHITE
        val DEFAULT_TEXT_COLOR = Color.BLACK
        const val DEFAULT_HEIGHT_DP = MiniAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP

        fun fromJson(json: String?): NavigationBarConfig? {
            if (json.isNullOrEmpty()) {
                // Return default config if JSON is missing/empty, assuming NavBar should exist but be hidden
                return NavigationBarConfig(hidden = true)
            }

            return try {
                val jsonObject = JSONObject(json)

                // Handle navigation style - if "custom", we should hide the standard nav bar
                val navStyle = jsonObject.optString("navigationStyle", "default")
                val isHidden = jsonObject.optBoolean("hidden", false) || navStyle == "custom"

                // Parse text style (black or white)
                val textStyle = jsonObject.optString("navigationBarTextStyle", "black")

                NavigationBarConfig(
                    hidden = isHidden,
                    navigationBarBackgroundColor = parseColor(jsonObject.optString("navigationBarBackgroundColor"), DEFAULT_BACKGROUND_COLOR),
                    navigationBarTextStyle = textStyle,
                    navigationBarTitleText = jsonObject.optString("navigationBarTitleText", ""),
                    navigationStyle = navStyle
                )
            } catch (e: Exception) {
                Log.e("NavBarConfig", "Error parsing NavigationBar config: ${e.message}")
                NavigationBarConfig(hidden = true) // Fallback to hidden on parse error
            }
        }

        private fun parseColor(colorString: String?, defaultColor: Int): Int {
            if (colorString.isNullOrEmpty()) return defaultColor
            return try {
                Color.parseColor(colorString)
            } catch (e: Exception) {
                Log.w("NavBarConfig", "Invalid color string '$colorString'. Using default.")
                defaultColor
            }
        }
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
        private const val TAG = "LiangXia.NavigationBar"
        // Default colors - internal access
        internal val DEFAULT_BACKGROUND_COLOR = Color.WHITE
        internal val DEFAULT_FRONT_COLOR = Color.BLACK // Default text/icon color
    }

    private val titleTextView: TextView
    private val loadingIndicator: ProgressBar
    private val backButton: ImageView // Changed to ImageView for better custom drawable

    // Store current colors
    private var currentBackgroundColor = DEFAULT_BACKGROUND_COLOR
    private var currentFrontColor = DEFAULT_FRONT_COLOR

    /**
     * Custom back button drawable that draws a chevron "<" shape
     */
    private inner class BackButtonDrawable : Drawable() {
        private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = currentFrontColor
            style = Paint.Style.STROKE
            strokeWidth = 1.8f * resources.displayMetrics.density  // Reduced stroke width
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }

        override fun draw(canvas: Canvas) {
            val width = bounds.width()
            val height = bounds.height()

            // Calculate points for the chevron shape
            val centerY = height / 2f

            // Make chevron smaller and more compact
            val startX = width * 0.55f
            val endX = width * 0.35f

            // Draw the chevron lines with smaller angles
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

    init {
        val density = resources.displayMetrics.density
        // Use the constant from MiniAppActivity's companion object
        val defaultHeightPx = (MiniAppActivity.DEFAULT_NAV_BAR_HEIGHT_DP * density).toInt()

        // Set layout params for the FrameLayout itself
        layoutParams = LayoutParams(LayoutParams.MATCH_PARENT, defaultHeightPx)
        setBackgroundColor(currentBackgroundColor)

        // Back Button setup (left side)
        val buttonSize = (44 * density).toInt()
        backButton = ImageView(context).apply {
            layoutParams = LayoutParams(buttonSize, buttonSize).apply {
                gravity = Gravity.CENTER_VERTICAL or Gravity.START
                marginStart = (4 * density).toInt()
            }
            setImageDrawable(BackButtonDrawable())
            contentDescription = "Back"
            visibility = View.GONE // Hidden by default
            setOnClickListener {
                // Just print log for back button click
                Log.d(TAG, "Back button clicked")
            }
        }
        addView(backButton)

        // Title TextView setup
        titleTextView = TextView(context).apply {
            layoutParams = LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.CENTER // Center the title
            }
            textAlignment = View.TEXT_ALIGNMENT_CENTER
            setTextColor(currentFrontColor)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 17f) // Default title size
            typeface = android.graphics.Typeface.create("sans-serif-medium", android.graphics.Typeface.NORMAL) // Medium weight font
            visibility = View.VISIBLE
        }
        addView(titleTextView)

        // Loading Indicator setup
        val progressBarSize = (24 * density).toInt() // Size for the progress bar
        loadingIndicator = ProgressBar(context, null, android.R.attr.progressBarStyleSmall).apply {
            layoutParams = LayoutParams(progressBarSize, progressBarSize).apply {
                gravity = Gravity.CENTER_VERTICAL or Gravity.START // Position left of title usually
                marginStart = (16 * density).toInt() // Add some margin
                // We might adjust this later relative to back button etc.
            }
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
                indeterminateDrawable?.colorFilter = android.graphics.BlendModeColorFilter(currentFrontColor, android.graphics.BlendMode.SRC_IN)
            } else {
                @Suppress("DEPRECATION")
                indeterminateDrawable?.setColorFilter(currentFrontColor, android.graphics.PorterDuff.Mode.SRC_IN)
            }
            visibility = View.GONE // Initially hidden
        }
        addView(loadingIndicator)
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

        // Updated color filter for loading indicator
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.Q) {
            loadingIndicator.indeterminateDrawable?.colorFilter = android.graphics.BlendModeColorFilter(currentFrontColor, android.graphics.BlendMode.SRC_IN)
        } else {
            @Suppress("DEPRECATION")
            loadingIndicator.indeterminateDrawable?.setColorFilter(currentFrontColor, android.graphics.PorterDuff.Mode.SRC_IN)
        }

        // Update back button color
        (backButton.drawable as? BackButtonDrawable)?.updateColor(currentFrontColor)
    }

    /**
     * Sets the visibility of the back button.
     *
     * @param visible Whether the back button should be visible.
     */
    fun setBackButtonVisible(visible: Boolean) {
        backButton.visibility = if (visible) View.VISIBLE else View.GONE
    }
}
