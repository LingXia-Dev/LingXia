Page({
  data: {
    queryString: "",
  },

  onLoad: async function (options) {
    const entries = Object.entries(options || {}).map(([key, value]) => ({
      key,
      value: Array.isArray(value) ? value.join(",") : String(value ?? ""),
    }));

    const queryString = entries.length
      ? entries.map(({ key, value }) => `${key}=${value}`).join("&")
      : "";

    await this.setData({
      queryString,
    });
  },

  sendPopupMessage: function (params) {
    const raw =
      params && typeof params.message === "string" ? params.message : "";
    const message = raw.trim();

    if (!message) {
      return;
    }

    this.getEventEmitter().emit("popupMessage", {
      message,
      timestamp: Date.now(),
    });

    lx.hidePopup();
  },
});
