package com.lingxia.lxapp.SameLevel.Components

import android.graphics.RectF
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import com.lingxia.lxapp.APIs.media.LxMediaCommand
import com.lingxia.lxapp.APIs.media.LxMediaEvent
import com.lingxia.lxapp.APIs.media.LxMediaObjectFit
import com.lingxia.lxapp.APIs.media.LxMediaPlayer
import com.lingxia.lxapp.APIs.media.LxMediaPlayerConfig
import com.lingxia.lxapp.APIs.media.LxMediaQuality
import com.lingxia.lxapp.APIs.media.LxMediaSource
import com.lingxia.lxapp.LxApp
import com.lingxia.lxapp.SameLevel.LxNativeComponent
import com.lingxia.lxapp.SameLevel.LxNativeComponentFactory

class VideoComponentFactory : LxNativeComponentFactory {
    override fun make(id: String, initialProps: Map<String, Any?>, eventSink: (Map<String, Any>) -> Unit) =
        VideoComponent(id, initialProps, eventSink)
}

class VideoComponent(
    override val id: String,
    initialProps: Map<String, Any?>,
    eventSink: (Map<String, Any>) -> Unit
) : LxNativeComponent {

    private var player: LxMediaPlayer? = null
    private var context: android.content.Context? = null
    private val eventSinkRef = eventSink
    private var lastFrame: RectF? = null

    override val view: View get() = player?.view ?: FrameLayout(context!!)

    override fun mount(host: ViewGroup) {
        context = LxApp.getCurrentActivity() ?: host.context
        player = LxMediaPlayer(context!!, eventSinkRef, typedEventSink = { event ->
            if (event is LxMediaEvent.FullscreenChange && !event.fullScreen) {
                lastFrame?.let { player?.setFrame(it.left, it.top, it.width(), it.height()) }
            }
        }, componentId = id)
        host.addView(player!!.view)
    }

    override fun update(props: Map<String, Any?>) {
        player?.update(makeConfig(props))
    }

    override fun setFrame(frame: RectF) {
        lastFrame = RectF(frame)
        if (player?.isFullscreen() != true) {
            player?.setFrame(frame.left, frame.top, frame.width(), frame.height())
        }
    }

    override fun focus() {
        lastFrame?.let { player?.setFrame(it.left, it.top, it.width(), it.height()) }
        player?.view?.requestLayout()
    }

    override fun blur() {}

    override fun handleCommand(name: String, params: Map<String, Any?>?) {
        val command = makeCommand(name, params) ?: return
        player?.handle(command)
    }

    override fun unmount() {
        player?.pause()
        player?.exitFullscreen()
        player?.detach()
        player = null
    }

    companion object {
        fun makeConfig(props: Map<String, Any?>): LxMediaPlayerConfig {
            val config = LxMediaPlayerConfig()

            // Parse source
            (props["source"] as? Map<*, *>)?.let { source ->
                val type = source["type"] as? String
                val value = source["value"] as? String
                if (type != null && value != null) {
                    config.source = when (type) {
                        "url" -> LxMediaSource.Url(value)
                        "file" -> LxMediaSource.FilePath(value)
                        "pipe" -> LxMediaSource.Pipe(value)
                        else -> null
                    }
                }
            }

            // Fallback to src if no source
            if (config.source == null) {
                (props["src"] as? String)?.let { config.src = it }
            }

            (props["poster"] as? String)?.let { config.poster = it }
            (props["autoplay"] as? Boolean)?.let { config.autoplay = it }
            (props["loop"] as? Boolean)?.let { config.loop = it }
            (props["muted"] as? Boolean)?.let { config.muted = it }
            (props["volume"] as? Number)?.let { config.volume = it.toDouble() }
            (props["controls"] as? Boolean)?.let { config.controls = it }
            (props["cornerRadius"] as? Number)?.let { config.cornerRadius = it.toDouble() }

            // Parse qualities
            (props["qualities"] as? List<*>)?.let { qualitiesList ->
                config.qualities = qualitiesList.mapNotNull { entry ->
                    val map = entry as? Map<*, *> ?: return@mapNotNull null
                    val label = map["label"] as? String ?: return@mapNotNull null
                    val url = map["url"] as? String
                    LxMediaQuality(label, url)
                }
            }

            // Parse speeds
            (props["speeds"] as? List<*>)?.let { speedsList ->
                config.speeds = speedsList.mapNotNull { (it as? Number)?.toDouble() }
            }

            (props["showControlsOnInit"] as? Boolean)?.let { config.showControlsOnInit = it }
            (props["objectFit"] as? String)?.let { config.objectFit = LxMediaObjectFit.fromString(it) }

            return config
        }

        fun makeCommand(name: String, params: Map<String, Any?>?): LxMediaCommand? {
            return when (name) {
                "play" -> LxMediaCommand.Play
                "pause" -> LxMediaCommand.Pause
                "stop" -> LxMediaCommand.Stop
                "seek" -> {
                    val time = (params?.get("time") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.Seek(time)
                }
                "setVolume" -> {
                    val volume = (params?.get("volume") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.SetVolume(volume)
                }
                "setMuted" -> {
                    val muted = params?.get("muted") as? Boolean ?: return null
                    LxMediaCommand.SetMuted(muted)
                }
                "setPlaybackRate" -> {
                    val rate = (params?.get("rate") as? Number)?.toDouble() ?: return null
                    LxMediaCommand.SetPlaybackRate(rate)
                }
                "enterFullscreen" -> LxMediaCommand.EnterFullscreen
                "exitFullscreen" -> LxMediaCommand.ExitFullscreen
                else -> null
            }
        }
    }
}
