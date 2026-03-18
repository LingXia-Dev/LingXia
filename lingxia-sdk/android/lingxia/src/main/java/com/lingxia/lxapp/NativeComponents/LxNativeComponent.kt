package com.lingxia.lxapp.NativeComponents

import android.graphics.RectF
import android.view.View
import android.view.ViewGroup

/**
 * Protocol for native components that can be rendered in component overlay.
 */
internal interface LxNativeComponent {
    val id: String
    val view: View

    fun mount(host: ViewGroup)
    fun update(props: Map<String, Any?>)
    fun setFrame(frame: RectF)
    fun focus()
    fun blur()
    fun handleCommand(name: String, params: Map<String, Any?>?)
    fun unmount()
}

/**
 * Factory protocol for creating native components.
 */
internal interface LxNativeComponentFactory {
    fun make(
        id: String,
        initialProps: Map<String, Any?>,
        eventSink: (Map<String, Any>) -> Unit
    ): LxNativeComponent
}

