Page({
  data: { videos: [] },
  videoContext: null,

  onLoad: function (options) {
    console.log("[NativeVideo] onLoad options:", options);

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
      console.log("[NativeVideo] createVideoContext success:", videoId);
      return this.videoContext;
    } catch (e) {
      console.error(
        "[NativeVideo] createVideoContext failed:",
        e.message || e,
      );
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
    const time =
      typeof position === "number" ? position : Number(position) || 0;
    this._getContext()?.seek(time);
  },

  requestFullScreen: function () {
    this._getContext()?.requestFullScreen();
  },

  exitFullScreen: function () {
    this._getContext()?.exitFullScreen();
  },

  onQualityChange: function ({ videoId, detail } = {}) {
    console.log("[NativeVideo] onQualityChange:", { videoId, detail });
  },

  onRateChange: function ({ videoId, detail } = {}) {
    console.log("[NativeVideo] onRateChange:", { videoId, detail });
  },
});
