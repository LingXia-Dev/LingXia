Page({
  data: {
    currentType: '',
    appBaseInfo: null,
    systemSetting: null
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

  getBaseInfo: async function () {
    try {
      const info = lx.app.getBaseInfo();
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
  },

  getSystemSetting: function () {
    try {
      const info = lx.getSystemSetting();
      console.log('System setting:', info);
      this.setData({
        systemSetting: info
      });
    } catch (error) {
      console.error('Failed to get system setting:', error);
      this.setData({
        systemSetting: null
      });
    }
  }
});
