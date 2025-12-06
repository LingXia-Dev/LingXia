Page({
  data: {
    expandedSections: {
      media: true, // Default expanded for visibility
    },
  },

  onLoad: function (options) {
    console.log("Components page onLoad options:", options);
  },

  onShow: function () {
    console.log("Components page onShow");
  },

  onHide: function () {
    console.log("Components page onHide");
  },

  toggleSection: function (params) {
    const { section } = params;
    const currentState = this.data.expandedSections[section];
    this.setData({
      [`expandedSections.${section}`]: !currentState,
    });
  },

  navigateToVideoDemo: async function () {
    await lx.navigateTo({
      url: "pages/video/index.tsx",
    });
  },
});