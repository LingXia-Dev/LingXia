Page({
  data: {
    currentType: '',
    appBaseInfo: null
  },

  onLoad: async function (options) {
    console.log('System page onLoad options:', options);
    this.setData({
      currentType: options.type || 'appBaseInfo'
    });
  },

  onShow: function () {
    console.log('System page onShow');
  },

  onHide: function () {
    console.log('System page onHide');
  },

  getAppBaseInfo: async function () {
    try {
      const info = lx.getAppBaseInfo();
      console.log('App base info:', info);
      this.setData({
        appBaseInfo: info
      });
    } catch (error) {
      console.error('Failed to get app base info:', error);
      this.setData({
        appBaseInfo: null
      });
    }
  }
});
