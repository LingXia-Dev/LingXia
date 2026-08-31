Page({
  data: {
    videos: [],
    eventLog: "Ready",
    currentTime: 0,
    duration: 0,
  },
  videoContext: null,
  usedLocalFallback: false,

  onLoad: function (options = {}) {
    if (options.automationFixture === "video-context-shape") {
      this.setData({
        videos: [{
          id: "lx-video-shape-fixture",
          src: "",
          poster: "",
          qualities: [],
          playbackRates: [1.0],
        }],
      });
      return;
    }
    // A deterministic local source (the automation HTTP fixture) so playback
    // commands can be asserted without the public internet; autoplay stays off
    // so the spec owns every transition.
    if (options.automationFixture === "video-source") {
      this.setData({
        videos: [{
          id: "lx-video-source-fixture",
          src: String(options.src || ""),
          poster: "",
          autoplay: false,
          qualities: [],
          playbackRates: [1.0],
        }],
      });
      return;
    }

    this.setData({
      videos: [
        {
          id: "lx-video-1",
          // Big Buck Bunny — (c) Blender Foundation, CC-BY 3.0,
          // https://peach.blender.org — served from Blender's official mirror.
          src: "public/island-sample.mp4",
          poster:
            "https://upload.wikimedia.org/wikipedia/commons/thumb/c/c5/Big_buck_bunny_poster_big.jpg/640px-Big_buck_bunny_poster_big.jpg",
          qualities: [
            {
              label: "1080P",
              url: "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_1080p_h264.mov",
            },
            {
              label: "720P",
              url: "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_720p_h264.mov",
            },
            {
              label: "480P",
              url: "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_480p_h264.mov",
            },
          ],
          playbackRates: [1.0, 0.5, 1.5, 2.0],
        },
      ],
    });
  },

  _getContext: function () {
    if (this.videoContext) return this.videoContext;
    const videoId = this.data?.videos?.[0]?.id;
    if (!videoId) return null;

    try {
      this.videoContext = lx.createVideoContext(videoId);
      return this.videoContext;
    } catch {
      return null;
    }
  },

  play: function () {
    try {
      this._getContext()?.play();
    } catch {
      /* island player may not be mounted yet */
    }
  },

  pause: function () {
    try {
      this._getContext()?.pause();
    } catch {
      /* island player may not be mounted yet */
    }
  },

  stop: function () {
    try {
      this._getContext()?.stop();
    } catch {
      /* island player may not be mounted yet */
    }
  },

  seek: function (position) {
    const time = typeof position === "number" ? position : Number(position) || 0;
    try {
      this._getContext()?.seek(time);
    } catch {
      /* island player may not be mounted yet */
    }
  },

  requestFullScreen: function () {
    this._getContext()?.requestFullScreen();
  },

  onError: function () {
    if (this.usedLocalFallback) return;
    this.usedLocalFallback = true;
    this.videoContext = null;
    this.setData({
      eventLog: "Fallback",
      videos: [{
        id: "lx-video-1",
        src: "public/island-sample.mp4",
        poster: "",
        qualities: [],
        playbackRates: [1.0],
      }],
    });
  },

  onPlaying: function () {
    try {
      this.setData({ eventLog: "Playing" });
    } catch {
      /* page may already be tearing down */
    }
  },

  onPause: function () {
    this.setData({ eventLog: "Paused" });
  },

  onStop: function () {
    this.setData({ eventLog: "Stopped" });
  },

  onEnded: function () {
    this.setData({ eventLog: "Ended" });
  },

  onWaiting: function () {
    this.setData({ eventLog: "Buffering..." });
  },

  onTimeUpdate: function (payload = {}) {
    const detail = payload?.detail || payload;
    const nextData = {};
    if (typeof detail.currentTime === "number") {
      nextData.currentTime = detail.currentTime;
    }
    if (typeof detail.duration === "number") {
      nextData.duration = detail.duration;
    }
    if (Object.keys(nextData).length > 0) {
      this.setData(nextData);
    }
  },

  onFullscreenChange: function (payload = {}) {
    const detail = payload?.detail || payload;
    const fullScreen = detail.fullScreen === true || detail.fullscreen === true;
    this.setData({ eventLog: `Fullscreen: ${fullScreen ? "on" : "off"}` });
  },

  onQualityChange: function (payload = {}) {
    const detail = payload?.detail || payload;
    this.setData({ eventLog: `Quality: ${detail.quality ?? detail.id ?? ""}` });
  },

  onRateChange: function (payload = {}) {
    const detail = payload?.detail || payload;
    this.setData({ eventLog: `Rate: ${detail.rate ?? ""}` });
  },
});
