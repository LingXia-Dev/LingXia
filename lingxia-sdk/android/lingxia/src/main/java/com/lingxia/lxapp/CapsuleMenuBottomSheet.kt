package com.lingxia.lxapp

import android.app.Activity
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import com.lingxia.lxapp.util.ActivityInsets

/**
 * Bottom sheet menu that appears when clicking the capsule menu (3-dots) button.
 * Shows LxApp info and action options in horizontal layout.
 */
internal object CapsuleMenuBottomSheet {
    private data class ReleaseBadge(
        val text: String,
        val textColor: Int,
        val backgroundColor: Int
    )


    private data class MenuItem(
        val iconResId: Int,
        val titleResId: Int,
        val action: String,
        val color: Int = Color.parseColor("#333333")
    )

    fun show(activity: Activity, appId: String) {
        val lxappInfo = NativeApi.getLxAppInfo(appId)
        if (lxappInfo == null) {
            return
        }

        val items = listOf(
            MenuItem(
                iconResId = R.drawable.icon_clean_cache,
                titleResId = R.string.lx_capsule_clean_cache,
                action = NativeApi.CAPSULE_ACTION_CLEAN_CACHE_RESTART
            ),
            MenuItem(
                iconResId = R.drawable.icon_restart,
                titleResId = R.string.lx_capsule_restart,
                action = NativeApi.CAPSULE_ACTION_RESTART
            ),
            MenuItem(
                iconResId = R.drawable.icon_uninstall,
                titleResId = R.string.lx_capsule_uninstall,
                action = NativeApi.CAPSULE_ACTION_UNINSTALL
            )
        )

        val rootView = activity.window.decorView as ViewGroup
        val container = FrameLayout(activity).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }

        // Create mask (semi-transparent overlay)
        val mask = createMaskView(activity) {
            // Dismiss on mask click
            rootView.removeView(container)
        }
        container.addView(mask)

        // Create menu content
        val menuView = createMenuView(activity, lxappInfo, items) { action ->
            // Send UI event for the selected action
            NativeApi.onLxappEvent(appId, NativeApi.UI_EVENT_CAPSULE_CLICK, action)
            // Dismiss the menu
            rootView.removeView(container)
        }
        container.addView(menuView)

        rootView.addView(container)
    }

    private fun createMaskView(context: Context, onClick: () -> Unit): FrameLayout {
        return FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#80000000")) // 50% black
            setOnClickListener { onClick() }
        }
    }

    private fun createMenuView(
        context: Context,
        lxappInfo: LxAppInfo,
        items: List<MenuItem>,
        onItemClick: (String) -> Unit
    ): FrameLayout {
        val density = context.resources.displayMetrics.density

        val menuContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = createMenuBackground(context)
            clipChildren = false
            clipToPadding = false
        }

        // Add header with app info
        val headerView = createHeaderView(context, lxappInfo)
        menuContainer.addView(headerView)

        // Add separator
        menuContainer.addView(createSeparatorView(context))

        // Add horizontal button row
        val buttonsRow = createButtonsRow(context, items, onItemClick)
        menuContainer.addView(buttonsRow)

        // Apply bottom inset for safe area
        val bottomInset = ActivityInsets.contentBottomInset()
        menuContainer.setPadding(
            (20 * density).toInt(),
            (16 * density).toInt(),
            (20 * density).toInt(),
            (16 * density + bottomInset).toInt()
        )

        // Wrap in FrameLayout for positioning
        return FrameLayout(context).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM
            )
            clipChildren = false
            clipToPadding = false
            addView(menuContainer)
        }
    }

    private fun createHeaderView(context: Context, lxappInfo: LxAppInfo): LinearLayout {
        val density = context.resources.displayMetrics.density

        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            clipChildren = false
            clipToPadding = false
            setPadding(
                0,
                0,
                0,
                (12 * density).toInt()
            )

            // App name
            addView(TextView(context).apply {
                text = lxappInfo.appName
                textSize = 16f
                setTextColor(Color.parseColor("#000000"))
                typeface = android.graphics.Typeface.DEFAULT_BOLD
            })

            // Separator
            addView(TextView(context).apply {
                text = " · "
                textSize = 16f
                setTextColor(Color.parseColor("#CCCCCC"))
                setPadding((4 * density).toInt(), 0, (4 * density).toInt(), 0)
            })

            // Version + badge container (to position badge at top-right)
            val versionContainer = FrameLayout(context).apply {
                clipChildren = false
                clipToPadding = false
            }

            // Version label with right padding for badge space
            val versionLabel = TextView(context).apply {
                text = lxappInfo.version
                textSize = 14f
                setTextColor(Color.parseColor("#999999"))
                // Reserve space on right for badge
                setPadding(0, 0, if (releaseBadgeFor(lxappInfo.releaseType) != null) (38 * density).toInt() else 0, 0)
            }
            versionContainer.addView(versionLabel)

            // Optional release type badge (positioned at top-right of version)
            releaseBadgeFor(lxappInfo.releaseType)?.let { badge ->
                val badgeView = TextView(context).apply {
                    text = badge.text
                    textSize = 10f
                    setTextColor(badge.textColor)
                    typeface = android.graphics.Typeface.DEFAULT_BOLD
                    gravity = Gravity.CENTER
                    setPadding(
                        (6 * density).toInt(),
                        (2 * density).toInt(),
                        (6 * density).toInt(),
                        (2 * density).toInt()
                    )
                    minWidth = (34 * density).toInt()
                    background = GradientDrawable().apply {
                        shape = GradientDrawable.RECTANGLE
                        cornerRadius = 8f * density
                        setColor(badge.backgroundColor)
                    }
                }
                val badgeParams = FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT,
                    (16 * density).toInt(),
                    Gravity.END or Gravity.TOP
                )
                badgeParams.topMargin = -(6 * density).toInt()
                badgeParams.rightMargin = (2 * density).toInt()
                badgeView.layoutParams = badgeParams
                versionContainer.addView(badgeView)
            }

            addView(versionContainer)
        }
    }

    private fun releaseBadgeFor(releaseType: String): ReleaseBadge? {
        return when (releaseType.lowercase()) {
            "developer" -> ReleaseBadge(
                text = "DEV",
                textColor = Color.parseColor("#1D4ED8"),
                backgroundColor = Color.parseColor("#DBEAFE")
            )

            "preview" -> ReleaseBadge(
                text = "PRE",
                textColor = Color.parseColor("#B45309"),
                backgroundColor = Color.parseColor("#FFEDD5")
            )

            else -> null
        }
    }

    private fun createButtonsRow(
        context: Context,
        items: List<MenuItem>,
        onClick: (String) -> Unit
    ): LinearLayout {
        val density = context.resources.displayMetrics.density

        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            setPadding(
                0,
                (12 * density).toInt(),
                0,
                0
            )

            items.forEachIndexed { index, item ->
                addView(createButtonView(context, item) { onClick(item.action) })

                // Add spacer between buttons (not after last)
                if (index < items.size - 1) {
                    addView(android.view.View(context).apply {
                        layoutParams = LinearLayout.LayoutParams(
                            (16 * density).toInt(),
                            1
                        )
                    })
                }
            }
        }
    }

    private fun createButtonView(
        context: Context,
        item: MenuItem,
        onClick: () -> Unit
    ): LinearLayout {
        val density = context.resources.displayMetrics.density

        return LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                0,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                1f
            )
            setPadding(
                (12 * density).toInt(),
                (12 * density).toInt(),
                (12 * density).toInt(),
                (12 * density).toInt()
            )
            setOnClickListener { onClick() }

            // Ripple effect on click
            val outValue = android.util.TypedValue()
            context.theme.resolveAttribute(
                android.R.attr.selectableItemBackground,
                outValue,
                true
            )
            setBackgroundResource(outValue.resourceId)

            // Icon
            addView(ImageView(context).apply {
                layoutParams = LinearLayout.LayoutParams(
                    (24 * density).toInt(),
                    (24 * density).toInt()
                ).apply {
                    bottomMargin = (6 * density).toInt()
                }
                setImageResource(item.iconResId)
                setColorFilter(item.color)
            })

            // Title
            addView(TextView(context).apply {
                text = context.getString(item.titleResId)
                textSize = 13f
                setTextColor(item.color)
                gravity = Gravity.CENTER
            })
        }
    }

    private fun createSeparatorView(context: Context): android.view.View {
        val density = context.resources.displayMetrics.density
        return android.view.View(context).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                maxOf(1, density.toInt())
            )
            setBackgroundColor(Color.parseColor("#EEEEEE"))
        }
    }

    private fun createMenuBackground(context: Context): GradientDrawable {
        val density = context.resources.displayMetrics.density
        val radius = 16f * density
        return GradientDrawable().apply {
            shape = GradientDrawable.RECTANGLE
            setColor(Color.WHITE)
            // Round only top corners
            cornerRadii = floatArrayOf(
                radius, radius, // top-left
                radius, radius, // top-right
                0f, 0f,         // bottom-right
                0f, 0f          // bottom-left
            )
        }
    }
}
