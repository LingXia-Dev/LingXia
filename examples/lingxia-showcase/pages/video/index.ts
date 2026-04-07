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
          src: "https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-576p.mp4",
          poster:
            "https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-HD.jpg",
          qualities: [
            {
              label: "1080P",
              url: "https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-1080p.mp4",
            },
            {
              label: "720P",
              url: "https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-720p.mp4",
            },
            {
              label: "576P",
              url: "https://cdn.plyr.io/static/demo/View_From_A_Blue_Moon_Trailer-576p.mp4",
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
