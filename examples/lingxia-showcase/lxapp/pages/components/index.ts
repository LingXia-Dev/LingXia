Page({
  data: {},

  onLoad: function (options) {
    console.log("Components page onLoad options:", options);
  },

  navigateTo: async function (params = {}) {
    const { page, query } = params;
    if (!page) {
      return;
    }
    if (query && typeof query === "object") {
      await lx.navigateTo({ page, query });
      return;
    }
    await lx.navigateTo({ page });
  },
});
