let nextItemId = 1;
function makeId() {
  const id = `picked-${nextItemId}`;
  nextItemId += 1;
  return id;
}

function clampIndex(value, count) {
  if (count <= 0) return 0;
  if (typeof value !== "number" || !Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(count - 1, Math.trunc(value)));
}

function eventDetail(event = {}) {
  if (event && typeof event === "object" && event.detail && typeof event.detail === "object") {
    return event.detail;
  }
  return event && typeof event === "object" ? event : {};
}

Page({
  data: {
    items: [],
    index: 0,
    autoplay: false,
    loop: false,
    dots: true,
    objectFit: "cover",
    animation: "slide",
    direction: "horizontal",
    peek: 0,
    eventLog: "Pick media to start",
    busy: false,
  },

  onLoad: function () {},

  pickImages: async function () {
    await this._addMedia(["image"], 9);
  },

  pickVideos: async function () {
    await this._addMedia(["video"], 3);
  },

  _addMedia: async function (mediaType, count) {
    if (this.data.busy) return;
    this.setData({ busy: true });
    try {
      const result = await lx.chooseMedia({
        mediaType,
        sourceType: ["album", "camera"],
        count,
        camera: "back",
        maxDuration: 60,
      });
      if (!Array.isArray(result) || result.length === 0) {
        this.setData({ busy: false });
        return;
      }
      const picked = result.map((entry) => ({
        id: makeId(),
        type: entry.fileType,
        src: entry.tempFilePath,
      }));
      const merged = [...(this.data.items || []), ...picked];
      this.setData({
        items: merged,
        eventLog: `added ${picked.length} ${picked.length === 1 ? "item" : "items"}`,
        busy: false,
      });
    } catch (error) {
      this.setData({ busy: false });
      lx.showToast({
        title: error?.message || "chooseMedia failed",
        icon: "none",
      });
    }
  },

  removeCurrent: function () {
    const items = this.data.items || [];
    if (items.length === 0) return;
    const current = this.data.index || 0;
    const next = items.filter((_, i) => i !== current);
    const adjusted = clampIndex(current, next.length);
    this.setData({
      items: next,
      index: adjusted,
      eventLog: next.length > 0 ? "removed" : "cleared",
    });
  },

  clearAll: function () {
    this.setData({
      items: [],
      index: 0,
      eventLog: "cleared",
    });
  },

  toggleAutoplay: function () {
    this.setData({
      autoplay: !this.data.autoplay,
      eventLog: !this.data.autoplay ? "autoplay on" : "autoplay off",
    });
  },

  toggleLoop: function () {
    this.setData({
      loop: !this.data.loop,
      eventLog: !this.data.loop ? "loop on" : "loop off",
    });
  },

  toggleDots: function () {
    this.setData({ dots: !this.data.dots });
  },

  setObjectFit: function (params = {}) {
    const fit = params?.fit;
    if (fit !== "cover" && fit !== "contain" && fit !== "fill") return;
    this.setData({ objectFit: fit });
  },

  setAnimation: function (params = {}) {
    const animation = params?.animation;
    if (animation !== "slide" && animation !== "none") return;
    this.setData({ animation });
  },

  setDirection: function (params = {}) {
    const direction = params?.direction;
    if (direction !== "horizontal" && direction !== "vertical") return;
    this.setData({ direction });
  },

  setPeek: function (params = {}) {
    const value = Number(params?.value);
    if (!Number.isFinite(value) || value < 0) return;
    this.setData({ peek: Math.round(value) });
  },

  onSwiperChange: function (event = {}) {
    const detail = eventDetail(event);
    if (typeof detail.index !== "number") return;
    // Friendly log: only emit a line when the index *actually* changes (some
    // race paths in native may still echo same-index transitions on rebuild).
    if (detail.index === this.data.index) return;
    this.setData({
      index: detail.index,
      eventLog: `Showing item ${detail.index + 1}`,
    });
  },

  onSwiperTransitionEnd: function (event = {}) {
    const detail = eventDetail(event);
    if (typeof detail.index === "number") {
      this.setData({ eventLog: `Settled on item ${detail.index + 1}` });
    }
  },

  onSwiperEndReached: function () {
    this.setData({ eventLog: "Reached the end" });
  },

  onSwiperTap: function (event = {}) {
    const detail = eventDetail(event);
    if (typeof detail.index === "number") {
      this.setData({ eventLog: `Tapped item ${detail.index + 1}` });
    }
  },

  onSwiperVideoEnded: function (event = {}) {
    const detail = eventDetail(event);
    const items = this.data.items || [];
    // Native does not auto-advance — JS owns the next-index decision so loop /
    // step semantics are policy-driven, not baked into the native swiper.
    if (typeof detail.index !== "number" || items.length === 0) {
      this.setData({ eventLog: "Video finished" });
      return;
    }
    if (items.length === 1) {
      this.setData({ eventLog: `Video finished (item ${detail.index + 1})` });
      return;
    }
    const last = items.length - 1;
    const atEnd = detail.index >= last;
    if (atEnd && !this.data.loop) {
      this.setData({ eventLog: `Video finished (last item)` });
      return;
    }
    const next = atEnd ? 0 : detail.index + 1;
    this.setData({
      index: next,
      eventLog: `Video finished — auto-advancing to item ${next + 1}`,
    });
  },

  onSwiperError: function (event = {}) {
    const detail = eventDetail(event);
    this.setData({ eventLog: `Playback error: ${detail.code || "unknown"}` });
  },
});
