Page({
  data: {},

  onLoad: function (options) {
    console.log("Components page onLoad options:", options);
  },

  navigateTo: async function (params) {
    const { url } = params;
    await lx.navigateTo({ url });
  },
});
