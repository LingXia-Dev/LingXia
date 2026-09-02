Page({
  data: {
    queryString: "",
    // Counts how many times the page-level onShow / onHide lifecycle hooks
    // have fired. surface.show()/hide() trigger these the same way navigation
    // away from a regular page would — the hide/show counters bump on each
    // visibility toggle, proving the lifecycle is wired end-to-end.
    showCount: 0,
    hideCount: 0,
    lastLifecycle: "onLoad",
    // Messages the opener pushed down through its PageSurface / PageMessagePort.
    inboundCount: 0,
    lastInbound: "",
  },
  _offInbound: [],

  onLoad: async function (options) {
    const entries = Object.entries(options || {}).map(([key, value]) => ({
      key,
      value: Array.isArray(value) ? value.join(",") : String(value ?? ""),
    }));

    const queryString = entries.length
      ? entries.map(({ key, value }) => `${key}=${value}`).join("&")
      : "";

    this.setData({
      queryString,
    });
    this._listenInbound();
  },

  onUnload: function () {
    this._stopInbound();
  },

  _listenInbound: function () {
    this._stopInbound();
    const receive = (message) => {
      const next = (this.data.inboundCount || 0) + 1;
      this.setData({
        inboundCount: next,
        lastInbound: JSON.stringify(message ?? null),
      });
    };
    // A page opened as a surface hears its opener on `surface`; one pushed by
    // navigateTo hears it on `opener`. Both are absent for a plain tab/stack page.
    const ports = [this.surface, this.opener].filter(Boolean);
    this._offInbound = ports.map((port) => port.onMessage(receive));
  },

  _stopInbound: function () {
    for (const off of this._offInbound || []) off();
    this._offInbound = [];
  },

  onShow: function () {
    console.log("surface page onShow");
    const next = (this.data.showCount || 0) + 1;
    this.setData({
      showCount: next,
      lastLifecycle: `onShow (#${next})`,
    });
  },

  onHide: function () {
    console.log("surface page onHide");
    const next = (this.data.hideCount || 0) + 1;
    this.setData({
      hideCount: next,
      lastLifecycle: `onHide (#${next})`,
    });
  },

  logSurfaceMessage: async function (params) {
    const raw =
      params && typeof params.message === "string" ? params.message : "";
    const message = raw.trim();

    if (!message) {
      return;
    }

    const payload = { message, timestamp: Date.now() };
    console.log("surface page message:", payload);
    if (this.surface) {
      this.surface.postMessage(payload);
      await this.closeSelf();
      return;
    }
    if (this.opener) {
      this.opener.postMessage(payload);
      await lx.navigateBack();
    }
  },

  hideSelf: async function () {
    try {
      await this.surface.hide();
    } catch (error) {
      console.warn("surface.hide failed:", error);
    }
  },

  closeSelf: async function () {
    try {
      await this.surface.close();
    } catch (error) {
      console.warn("surface.close failed:", error);
    }
  },
});
