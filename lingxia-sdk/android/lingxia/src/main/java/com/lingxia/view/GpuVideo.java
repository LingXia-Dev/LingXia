package com.lingxia.view;

import android.app.Application;
import android.content.Context;
import android.graphics.SurfaceTexture;
import android.net.Uri;
import android.os.Handler;
import android.os.HandlerThread;
import android.util.Log;
import android.view.Surface;
import androidx.media3.common.MediaItem;
import androidx.media3.common.Player;
import androidx.media3.common.VideoSize;
import androidx.media3.exoplayer.ExoPlayer;
import java.io.File;

/**
 * One video the GPU renderer draws: ExoPlayer decodes into a detached SurfaceTexture that the
 * render thread attaches to its GL context and latches with {@link #update}. Rust calls every
 * method; only {@link #attach} and {@link #update} must run on the render thread.
 */
final class GpuVideo implements SurfaceTexture.OnFrameAvailableListener {
    private static final String TAG = "LingXia.UI";
    private static HandlerThread thread;

    private final Handler handler;
    private final SurfaceTexture texture = new SurfaceTexture(false);
    private final Surface surface = new Surface(texture);
    private final float[] matrix = new float[16];
    private ExoPlayer player;
    private volatile boolean available;
    private volatile int width;
    private volatile int height;
    private boolean attached;

    static GpuVideo open(String source) {
        return new GpuVideo(source);
    }

    private static synchronized Handler handler() {
        if (thread == null) {
            thread = new HandlerThread("lingxia-video");
            thread.start();
        }
        return new Handler(thread.getLooper());
    }

    private GpuVideo(String source) {
        handler = handler();
        texture.setOnFrameAvailableListener(this, handler);
        Context context = application();
        Uri uri = resolve(context, source);
        handler.post(() -> {
            // Built on the handler thread, which becomes the player's application thread.
            player = new ExoPlayer.Builder(context).build();
            player.setVideoSurface(surface);
            player.setRepeatMode(Player.REPEAT_MODE_ALL);
            player.setVolume(0f);
            player.addListener(new Player.Listener() {
                @Override
                public void onVideoSizeChanged(VideoSize size) {
                    width = size.width;
                    height = size.height;
                }
            });
            player.setMediaItem(MediaItem.fromUri(uri));
            player.prepare();
            player.play();
        });
    }

    @Override
    public void onFrameAvailable(SurfaceTexture ignored) {
        available = true;
    }

    boolean available() {
        return available;
    }

    int width() {
        return width;
    }

    int height() {
        return height;
    }

    void attach(int name) {
        if (!attached) {
            texture.attachToGLContext(name);
            attached = true;
        }
    }

    /** Latches the newest frame into the attached texture; its transform, or null with none. */
    float[] update() {
        if (!attached || !available) {
            return null;
        }
        available = false;
        texture.updateTexImage();
        texture.getTransformMatrix(matrix);
        return matrix;
    }

    void close() {
        handler.post(() -> {
            if (player != null) {
                player.release();
                player = null;
            }
            surface.release();
            texture.release();
        });
    }

    private static Uri resolve(Context context, String source) {
        if (source.startsWith("local:")) {
            String name = source.substring("local:".length());
            File external = context.getExternalFilesDir(null);
            File file = external != null ? new File(external, name) : null;
            if (file == null || !file.exists()) {
                file = new File(context.getFilesDir(), name);
            }
            return Uri.fromFile(file);
        }
        return Uri.parse(source);
    }

    private static Context application() {
        try {
            Application application = (Application) Class.forName("android.app.ActivityThread")
                .getMethod("currentApplication")
                .invoke(null);
            return application;
        } catch (ReflectiveOperationException error) {
            Log.w(TAG, "GpuVideo: no application", error);
            throw new IllegalStateException(error);
        }
    }
}
