Page({
  data: {
    videos: [],
    eventLog: "Ready",
    currentTime: 0,
    duration: 0,
  },
  videoContext: null,

  onLoad: function () {
    this.setData({
      videos: [
        {
          id: "lx-video-1",
          // Big Buck Bunny — (c) Blender Foundation, CC-BY 3.0,
          // https://peach.blender.org — served from Blender's official mirror.
          src: "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_480p_h264.mov",
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
    this._getContext()?.play();
  },

  pause: function () {
    this._getContext()?.pause();
  },

  stop: function () {
    this._getContext()?.stop();
  },

  seek: function (position) {
    const time = typeof position === "number" ? position : Number(position) || 0;
    this._getContext()?.seek(time);
  },

  requestFullScreen: function () {
    this._getContext()?.requestFullScreen();
  },

  onPlaying: function () {
    this.setData({ eventLog: "Playing" });
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

  onTimeUpdate: function (event = {}) {
    const detail = event?.detail || {};
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

  onFullscreenChange: function (event = {}) {
    const fullScreen = event?.detail?.fullScreen === true;
    this.setData({ eventLog: `Fullscreen: ${fullScreen ? "on" : "off"}` });
  },

  onQualityChange: function (event = {}) {
    const detail = event?.detail || {};
    this.setData({ eventLog: `Quality: ${detail.quality ?? ""}` });
  },

  onRateChange: function (event = {}) {
    const detail = event?.detail || {};
    this.setData({ eventLog: `Rate: ${detail.rate ?? ""}` });
  },
});
