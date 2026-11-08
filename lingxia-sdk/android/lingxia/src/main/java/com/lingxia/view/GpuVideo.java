package com.lingxia.view;

import android.app.Application;
import android.content.Context;
import android.graphics.SurfaceTexture;
import android.net.Uri;
import android.os.Handler;
import android.os.HandlerThread;
import android.util.Log;
import android.view.Surface;
import androidx.media3.common.C;
import androidx.media3.common.MediaItem;
import androidx.media3.common.PlaybackException;
import androidx.media3.common.PlaybackParameters;
import androidx.media3.common.Player;
import androidx.media3.common.Tracks;
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
    /** The transport as the handler thread last saw it, so Rust reads it from any thread. */
    private volatile double[] snapshot = {0, -1, 1, 0, 0, 0, 1, 1, 0, 0, 0};
    private volatile String error;
    private boolean ended;
    private boolean muted;
    private float volume = 1f;
    /** An adaptive stream's video heights, highest first, as "720p,360p". */
    private volatile String qualities = "";
    private static final long SAMPLE_MS = 100;

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
            player.addListener(new Player.Listener() {
                @Override
                public void onVideoSizeChanged(VideoSize size) {
                    width = size.width;
                    height = size.height;
                }

                @Override
                public void onPlaybackStateChanged(int state) {
                    if (state == Player.STATE_ENDED && player != null) {
                        ended = true;
                        player.setPlayWhenReady(false);
                    }
                    sample();
                }

                @Override
                public void onTracksChanged(Tracks tracks) {
                    java.util.TreeSet<Integer> heights = new java.util.TreeSet<>(java.util.Collections.reverseOrder());
                    for (Tracks.Group group : tracks.getGroups()) {
                        if (group.getType() != C.TRACK_TYPE_VIDEO || !group.isAdaptiveSupported()) {
                            continue;
                        }
                        for (int i = 0; i < group.length; i++) {
                            int height = group.getTrackFormat(i).height;
                            if (height > 0 && group.isTrackSupported(i)) {
                                heights.add(height);
                            }
                        }
                    }
                    StringBuilder text = new StringBuilder();
                    for (int height : heights) {
                        text.append(text.length() == 0 ? "" : ",").append(height).append('p');
                    }
                    qualities = text.toString();
                }

                @Override
                public void onPlayerError(PlaybackException failure) {
                    error = code(failure) + ": " + failure.getMessage();
                    sample();
                }
            });
            player.setMediaItem(MediaItem.fromUri(uri));
            player.prepare();
            sampleLoop();
        });
    }

    /** play, pause, seek (seconds), muted and loop (0/1), volume, rate; in order on the handler. */
    void command(String op, double value) {
        handler.post(() -> {
            if (player == null) {
                return;
            }
            if (op.equals("play")) {
                setPlaying(true);
            } else if (op.equals("pause")) {
                setPlaying(false);
            } else if (op.equals("seek")) {
                ended = false;
                player.seekTo((long) (value * 1000));
            } else if (op.equals("muted")) {
                muted = value != 0;
                player.setVolume(muted ? 0f : volume);
            } else if (op.equals("volume")) {
                volume = (float) value;
                player.setVolume(muted ? 0f : volume);
            } else if (op.equals("quality")) {
                pinHeight((int) value);
            } else if (op.equals("loop")) {
                player.setRepeatMode(value != 0 ? Player.REPEAT_MODE_ONE : Player.REPEAT_MODE_OFF);
            } else if (op.equals("rate")) {
                player.setPlaybackParameters(new PlaybackParameters((float) value));
            }
            sample();
        });
    }

    /** Pins the video rendition of this height; 0 gives adaptive selection back. Needs no tracks yet. */
    private void pinHeight(int height) {
        androidx.media3.common.TrackSelectionParameters.Builder params =
            player.getTrackSelectionParameters().buildUpon().clearOverridesOfType(C.TRACK_TYPE_VIDEO);
        if (height > 0) {
            params.setMinVideoSize(0, height).setMaxVideoSize(Integer.MAX_VALUE, height);
        } else {
            params.clearVideoSizeConstraints().setMinVideoSize(0, 0);
        }
        player.setTrackSelectionParameters(params.build());
    }

    String qualities() {
        return qualities;
    }

    private void setPlaying(boolean playing) {
        if (playing && ended) {
            ended = false;
            player.seekTo(0);
        }
        player.setPlayWhenReady(playing);
        sample();
    }

    /** currentTime, duration (-1 unknown), paused, ended, buffering, buffered, rate, volume, muted, seeking, ready. */
    double[] status() {
        return snapshot;
    }

    String error() {
        return error;
    }

    private void sampleLoop() {
        if (player == null) {
            return;
        }
        sample();
        handler.postDelayed(this::sampleLoop, SAMPLE_MS);
    }

    private void sample() {
        if (player == null) {
            return;
        }
        long duration = player.getDuration();
        int state = player.getPlaybackState();
        snapshot = new double[] {
            Math.max(0, player.getCurrentPosition()) / 1000.0,
            duration == C.TIME_UNSET ? -1 : duration / 1000.0,
            player.getPlayWhenReady() ? 0 : 1,
            ended ? 1 : 0,
            state == Player.STATE_BUFFERING && player.getPlayWhenReady() ? 1 : 0,
            Math.max(0, player.getBufferedPosition() - player.getCurrentPosition()) / 1000.0,
            player.getPlaybackParameters().speed,
            volume,
            muted ? 1 : 0,
            0,
            state == Player.STATE_READY || state == Player.STATE_ENDED ? 1 : 0,
        };
    }

    private static String code(PlaybackException failure) {
        int code = failure.errorCode;
        if (code >= 2000 && code < 3000) {
            return code == PlaybackException.ERROR_CODE_IO_FILE_NOT_FOUND ? "VIDEO_NOT_FOUND" : "VIDEO_NETWORK";
        }
        if (code >= 3000 && code < 4000) {
            return "VIDEO_FORMAT";
        }
        if (code >= 4000 && code < 5000) {
            return code == PlaybackException.ERROR_CODE_DECODER_INIT_FAILED ? "VIDEO_DECODERS" : "VIDEO_DECODE";
        }
        return "VIDEO_SOURCE";
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
