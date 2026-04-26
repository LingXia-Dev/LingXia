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

    this.setData({
      queryString,
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
    this.surface.postMessage(payload);
    await this.surface.close();
  },
});
