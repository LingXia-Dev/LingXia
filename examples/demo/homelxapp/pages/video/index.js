Page({
  data: {},
  videoContext: null,

  onLoad: function (options) {
    console.log("[VideoSameLevel] onLoad options:", options);
  },

  onShow: function () {
    console.log("[VideoSameLevel] onShow");
  },

  onHide: function () {
    console.log("[VideoSameLevel] onHide");
  },

  // Ensure videoContext exists, create if needed
  _ensureContext: function () {
    if (!this.videoContext) {
      try {
        this.videoContext = lx.createVideoContext("lx-video");
        console.log("[VideoSameLevel] createVideoContext success");
      } catch (e) {
        console.error("[VideoSameLevel] createVideoContext failed:", e.message);
        return null;
      }
    }
    return this.videoContext;
  },

  play: function () {
    console.log("[VideoSameLevel] play");
    try {
      this._ensureContext()?.play();
    } catch (e) {
      console.error("[VideoSameLevel] play failed:", e.message || e);
    }
  },

  pause: function () {
    console.log("[VideoSameLevel] pause");
    try {
      this._ensureContext()?.pause();
    } catch (e) {
      console.error("[VideoSameLevel] pause failed:", e.message || e);
    }
  },

  stop: function () {
    console.log("[VideoSameLevel] stop");
    try {
      this._ensureContext()?.stop();
    } catch (e) {
      console.error("[VideoSameLevel] stop failed:", e.message || e);
    }
  },

  seek: function (position) {
    console.log("[VideoSameLevel] seek to:", position);
    try {
      this._ensureContext()?.seek(position);
    } catch (e) {
      console.error("[VideoSameLevel] seek failed:", e.message || e);
    }
  },

  requestFullScreen: function () {
    console.log("[VideoSameLevel] requestFullScreen");
    try {
      this._ensureContext()?.requestFullScreen();
    } catch (e) {
      console.error(
        "[VideoSameLevel] requestFullScreen failed:",
        e.message || e,
      );
    }
  },

  exitFullScreen: function () {
    console.log("[VideoSameLevel] exitFullScreen");
    try {
      this._ensureContext()?.exitFullScreen();
    } catch (e) {
      console.error("[VideoSameLevel] exitFullScreen failed:", e.message || e);
    }
  },
});
