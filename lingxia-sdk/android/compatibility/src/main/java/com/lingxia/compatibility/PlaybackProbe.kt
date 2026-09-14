package com.lingxia.compatibility

import android.content.Context
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.PlaybackException
import androidx.media3.exoplayer.ExoPlayer
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicReference

/** Runs in the target APK so R8 sees playback's actual entry points. */
class PlaybackProbe(context: Context, uri: String) {
    val ended = CountDownLatch(1)
    val failure = AtomicReference<Throwable>()
    private val player = ExoPlayer.Builder(context).build()

    init {
        player.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(state: Int) {
                if (state == Player.STATE_ENDED) ended.countDown()
            }
            override fun onPlayerError(error: PlaybackException) {
                failure.set(error)
                ended.countDown()
            }
        })
        player.setMediaItem(MediaItem.fromUri(uri))
        player.prepare()
        player.play()
    }

    fun close() = player.release()
}
