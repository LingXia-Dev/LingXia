Page({
  data: {},

  onLoad: function (options) {
    console.log("Components page onLoad options:", options);
  },

  navigateTo: async function (params = {}) {
    const { url } = params;
    if (!url) {
      return;
    }
    await lx.navigateTo({ url });
  },
});
