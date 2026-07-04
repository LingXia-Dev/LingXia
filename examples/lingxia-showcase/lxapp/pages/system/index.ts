Page({
  data: {
    currentType: '',
    appBaseInfo: null,
    systemSetting: null,
    autostartSupported: false,
    autostartEnabled: null,
    autostartError: ''
  },

  onLoad: async function (options) {
    console.log('System page onLoad options:', options);
    this.setData({
      currentType: options.type || 'appBaseInfo'
    });
    if (options.type === 'autostart') {
      await this.refreshAutostart();
    }
  },

  onShow: function () {
    console.log('System page onShow');
    // The user can flip the login item in System Settings / Task Manager
    // while this page is hidden — re-read the OS state on every show.
    if (this.data.currentType === 'autostart') {
      this.refreshAutostart();
    }
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

  // lx.app.autostart is absent off macOS/Windows or without the capability,
  // so presence of the member is the support check.
  refreshAutostart: async function () {
    const autostart = lx.app.autostart;
    if (!autostart) {
      this.setData({ autostartSupported: false, autostartEnabled: null });
      return;
    }
    try {
      const enabled = await autostart.isEnabled();
      console.log('Autostart enabled:', enabled);
      this.setData({ autostartSupported: true, autostartEnabled: enabled, autostartError: '' });
    } catch (error) {
      console.error('Failed to read autostart state:', error);
      // Drop the stale value: rendering the old state after a failed re-read
      // would make the next toggle invert against reality.
      this.setData({ autostartSupported: true, autostartEnabled: null, autostartError: String(error) });
    }
  },

  toggleAutostart: async function () {
    const autostart = lx.app.autostart;
    if (!autostart) {
      return;
    }
    if (this.data.autostartEnabled === null) {
      await this.refreshAutostart();
      if (this.data.autostartEnabled === null) {
        return;
      }
    }
    const next = !this.data.autostartEnabled;
    try {
      await autostart.setEnabled(next);
      const enabled = await autostart.isEnabled();
      console.log('Autostart set to', next, '- OS reports', enabled);
      this.setData({ autostartEnabled: enabled, autostartError: '' });
    } catch (error) {
      console.error('Failed to toggle autostart:', error);
      this.setData({ autostartError: String(error) });
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
