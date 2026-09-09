Page({
  data: {
    currentType: 'appBaseInfo',
    appBaseInfo: null,
    displayLanguage: '',
    systemSetting: null,
    autostartSupported: false,
    autostartEnabled: null,
    autostartError: '',
    cacheBytes: null,
    cacheFreedBytes: null,
    cacheBusy: false,
    cacheError: '',
    cacheNotice: ''
  },

  onLoad: async function (options = {}) {
    console.log('System page onLoad options:', options);
    const { type = 'appBaseInfo' } = (options || {}) as { type?: string };
    if (type !== this.data.currentType) {
      this.setData({ currentType: type });
    }
    if (type === 'autostart') {
      await this.refreshAutostart();
    }
    if (type === 'cache') {
      await this.refreshCacheSize();
    }
  },

  onShow: function () {
    console.log('System page onShow');
    // The user can flip the login item in System Settings / Task Manager
    // while this page is hidden — re-read the OS state on every show.
    if (this.data.currentType === 'autostart') {
      this.refreshAutostart();
    }
    if (this.data.currentType === 'cache') {
      this.refreshCacheSize();
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
        appBaseInfo: info,
        // Identity is fixed for the process; the language is not, so it comes
        // from its own namespace rather than the identity snapshot.
        displayLanguage: lx.app.displayLanguage.get()
      });
    } catch (error) {
      console.error('Failed to get app base info:', error);
      this.setData({
        appBaseInfo: null,
        displayLanguage: ''
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

  // `lx.app.cache` is the whole product's cache, not this lxapp's, which is why
  // it is injected only into the Control app. The showcase *is* the Control
  // app, so the member is present here; a guest does not have it.
  refreshCacheSize: async function () {
    const cache = lx.app.cache;
    if (!cache) {
      this.setData({ cacheBytes: null, cacheError: 'Cache APIs are Control-app only' });
      return;
    }
    try {
      const bytes = await cache.size();
      console.log('Product cache size (bytes):', bytes);
      this.setData({ cacheBytes: bytes, cacheError: '' });
    } catch (error) {
      console.error('Failed to read cache size:', error);
      this.setData({ cacheBytes: null, cacheError: String(error) });
    }
  },

  clearCache: async function () {
    if (this.data.cacheBusy) {
      return;
    }
    this.setData({ cacheBusy: true, cacheError: '', cacheNotice: '' });
    const cache = lx.app.cache;
    if (!cache) {
      this.setData({ cacheBusy: false, cacheError: 'Cache APIs are Control-app only' });
      return;
    }
    try {
      const result = await cache.clear();
      const webview = result.webview === 'cleared'
        ? 'WebView cache cleared.'
        : result.webview === 'unsupported'
          ? 'WebView cache clearing is unavailable on this platform.'
          : 'WebView cache clearing failed.';
      this.setData({
        cacheFreedBytes: result.freedBytes,
        cacheNotice: result.skippedActivePaths > 0
          ? `Running apps retained their private caches. ${webview}`
          : webview,
        cacheError: result.failures.join('\n'),
      });
      const bytes = await cache.size();
      this.setData({ cacheBytes: bytes, cacheBusy: false });
    } catch (error) {
      console.error('Failed to clear cache:', error);
      this.setData({ cacheBusy: false, cacheError: String(error) });
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
