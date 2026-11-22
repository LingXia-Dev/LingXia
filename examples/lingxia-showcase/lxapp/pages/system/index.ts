import {
  definePage,
  lx,
  type HostInfo,
  type HostTarget,
  type SystemSettingInfo,
  type PageLoadOptions,
} from '@lingxia/logic';

export default definePage({
  data: {
    currentType: 'appBaseInfo',
    appBaseInfo: null as HostInfo | null,
    displayLanguage: '',
    systemSetting: null as SystemSettingInfo | null,
    autostartSupported: false,
    autostartEnabled: null as boolean | null,
    autostartError: '',
    cacheBytes: null as number | null,
    cacheFreedBytes: null as number | null,
    cacheBusy: false,
    cacheError: '',
    cacheNotice: '',
    notificationSupported: false,
    notificationPermission: '',
    notificationLastId: '',
    notificationTarget: 'activate',
    notificationError: '',
    bannerSupported: false,
    bannerLast: '',
    bannerError: '',
    bannerBusy: false,
    bannerBackground: 'system',
    bannerActiveId: '',
  },
  setup(page) {
    // An action switching section loads the page again.
    async function load(options: PageLoadOptions = {}) {
      console.log('System page onLoad options:', options);
      const { type = 'appBaseInfo' } = (options || {}) as { type?: string };
      if (type !== page.data.currentType) {
        page.update(d => { d.currentType = type; });
      }
      if (type === 'autostart') {
        await actions.refreshAutostart();
      }
      if (type === 'cache') {
        await actions.refreshCacheSize();
      }
      if (type === 'notification') {
        await actions.refreshNotification();
      }
      if (type === 'banner') {
        actions.refreshBanner();
      }
    }

    const actions = {
      async selectSection({ type }: { type: string }) { await load({ type }); },
      async getBaseInfo() {
        try {
          const info = lx.host.info;
          console.log('App base info:', info);
          page.update(d => {
            d.appBaseInfo = info;
            d.displayLanguage = lx.app.language.get();
          });
        } catch (error) {
          console.error('Failed to get app base info:', error);
          page.update(d => {
            d.appBaseInfo = null;
            d.displayLanguage = '';
          });
        }
      },
      // lx.host.autostart is always present; off macOS/Windows or without the
      // capability its calls fail, so ask lx.supports first.
      async refreshAutostart() {
        const autostart = lx.host.autostart;
        if (!lx.supports('host.autostart')) {
          page.update(d => {
            d.autostartSupported = false;
            d.autostartEnabled = null;
          });
          return;
        }
        try {
          const enabled = await autostart.isEnabled();
          console.log('Autostart enabled:', enabled);
          page.update(d => {
            d.autostartSupported = true;
            d.autostartEnabled = enabled;
            d.autostartError = '';
          });
        } catch (error) {
          console.error('Failed to read autostart state:', error);
          // Drop the stale value: rendering the old state after a failed re-read
          // would make the next toggle invert against reality.
          page.update(d => {
            d.autostartSupported = true;
            d.autostartEnabled = null;
            d.autostartError = String(error);
          });
        }
      },
      // `lx.host.cache` is the whole product's cache, not this lxapp's, which is why
      // it is injected only into the Control app. The showcase *is* the Control
      // app, so the member is present here; a guest does not have it.
      async refreshCacheSize() {
        const cache = lx.host.cache;
        if (lx.app.info.role !== 'control') {
          page.update(d => {
            d.cacheBytes = null;
            d.cacheError = lx.app.t('system.cacheControlOnly');
          });
          return;
        }
        try {
          const bytes = await cache.size();
          console.log('Product cache size (bytes):', bytes);
          page.update(d => {
            d.cacheBytes = bytes;
            d.cacheError = '';
          });
        } catch (error) {
          console.error('Failed to read cache size:', error);
          page.update(d => {
            d.cacheBytes = null;
            d.cacheError = String(error);
          });
        }
      },
      async clearCache() {
        if (page.data.cacheBusy) {
          return;
        }
        page.update(d => {
          d.cacheBusy = true;
          d.cacheError = '';
          d.cacheNotice = '';
        });
        const cache = lx.host.cache;
        if (lx.app.info.role !== 'control') {
          page.update(d => {
            d.cacheBusy = false;
            d.cacheError = lx.app.t('system.cacheControlOnly');
          });
          return;
        }
        try {
          const result = await cache.clear();
          const webview = result.webview === 'cleared'
            ? 'WebView cache cleared.'
            : result.webview === 'unsupported'
              ? 'WebView cache clearing is unavailable on this platform.'
              : 'WebView cache clearing failed.';
          page.update(d => {
            d.cacheFreedBytes = result.freedBytes;
            d.cacheNotice = result.skippedActivePaths > 0
              ? `Running apps retained their private caches. ${webview}`
              : webview;
            d.cacheError = result.failures.join('\n');
          });
          const bytes = await cache.size();
          page.update(d => {
            d.cacheBytes = bytes;
            d.cacheBusy = false;
          });
        } catch (error) {
          console.error('Failed to clear cache:', error);
          page.update(d => {
            d.cacheBusy = false;
            d.cacheError = String(error);
          });
        }
      },
      // Presence of the member is the support check 鈥?same latch as
      // `lx.supports('host.notification')`.
      async refreshNotification() {
        const notification = lx.host.notification;
        if (!lx.supports('host.notification')) {
          page.update(d => {
            d.notificationSupported = false;
            d.notificationPermission = '';
            d.notificationError = '';
          });
          return;
        }
        try {
          // Reading must not prompt; `show` asks when it needs to.
          const permission = await notification.permission();
          page.update(d => {
            d.notificationSupported = true;
            d.notificationPermission = permission;
            d.notificationError = '';
          });
        } catch (error) {
          console.error('Failed to read notification permission:', error);
          page.update(d => {
            d.notificationSupported = true;
            d.notificationPermission = '';
            d.notificationError = String(error);
          });
        }
      },
      setNotificationTarget(target: string) {
        page.update(d => { d.notificationTarget = target; });
      },
      async showNotification() {
        const notification = lx.host.notification;
        if (!lx.supports('host.notification')) {
          page.update(d => { d.notificationError = lx.app.t('system.noNotifications'); });
          return;
        }
        try {
          // Scheduled, so it reaches the OS even while this page is frontmost.
          const { id } = await notification.show({
            id: 'showcase-local',
            title: lx.app.t('system.showcaseTitle'),
            message: notificationBody(page.data.notificationTarget),
            target: notificationTargetFor(page.data.notificationTarget),
            schedule: { delayMs: 5000 },
          });
          page.update(d => {
            d.notificationLastId = id;
            d.notificationError = '';
          });
        } catch (error) {
          console.error('Failed to show notification:', error);
          page.update(d => { d.notificationError = String(error); });
        }
      },
      async cancelNotification() {
        const notification = lx.host.notification;
        if (!lx.supports('host.notification')) {
          return;
        }
        try {
          if (page.data.notificationLastId) {
            await notification.cancel(page.data.notificationLastId);
          } else {
            await notification.cancelAll();
          }
          page.update(d => {
            d.notificationLastId = '';
            d.notificationError = '';
          });
        } catch (error) {
          console.error('Failed to cancel notification:', error);
          page.update(d => { d.notificationError = String(error); });
        }
      },
      refreshBanner() {
        const banner = lx.host.banner;
        const supported = lx.supports('host.banner');
        page.update(d => {
          d.bannerSupported = supported;
          d.bannerError = banner ? '' : lx.app.t('system.bannerControlOnly');
        });
      },
      setBannerBackground(background: string) {
        page.update(d => { d.bannerBackground = background; });
      },
      async showBannerToast() {
        const banner = lx.host.banner;
        if (!lx.supports('host.banner')) {
          page.update(d => { d.bannerError = lx.app.t('system.noBanner'); });
          return;
        }
        if (page.data.bannerBusy) {
          return;
        }
        page.update(d => {
          d.bannerBusy = true;
          d.bannerError = '';
          d.bannerLast = '';
          d.bannerActiveId = 'showcase-banner-toast';
        });
        try {
          const result = await banner.show({
            id: 'showcase-banner-toast',
            title: lx.app.t('system.showcaseTitle'),
            message: lx.app.t('system.bannerToastBody'),
            durationMs: 5000,
            background: page.data.bannerBackground,
          });
          page.update(d => {
            d.bannerBusy = false;
            d.bannerLast = (result.status === 'canceled')
              ? `${result.reason}`
              : `action:${result.action}`;
          });
        } catch (error) {
          console.error('Failed to show banner toast:', error);
          page.update(d => {
            d.bannerBusy = false;
            d.bannerError = String(error);
          });
        }
      },
      async showBannerPrompt() {
        const banner = lx.host.banner;
        if (!lx.supports('host.banner')) {
          page.update(d => { d.bannerError = lx.app.t('system.noBanner'); });
          return;
        }
        if (page.data.bannerBusy) {
          return;
        }
        page.update(d => {
          d.bannerBusy = true;
          d.bannerError = '';
          d.bannerLast = '';
          d.bannerActiveId = 'showcase-banner-prompt';
        });
        try {
          const result = await banner.show({
            id: 'showcase-banner-prompt',
            title: lx.app.t('system.bannerPromptTitle'),
            message: lx.app.t('system.bannerPromptBody'),
            actions: [
              { id: 'deny', label: lx.app.t('system.deny') },
              { id: 'allow', label: lx.app.t('system.allow'), tone: 'primary' },
            ],
            durationMs: 60_000,
            background: page.data.bannerBackground,
          });
          page.update(d => {
            d.bannerBusy = false;
            d.bannerLast = (result.status === 'canceled')
              ? `${result.reason}`
              : `action:${result.action}`;
          });
        } catch (error) {
          console.error('Failed to show banner prompt:', error);
          page.update(d => {
            d.bannerBusy = false;
            d.bannerError = String(error);
          });
        }
      },
      async dismissBanner() {
        const banner = lx.host.banner;
        if (!lx.supports('host.banner')) {
          return;
        }
        try {
          const ids = [
            page.data.bannerActiveId,
            'showcase-banner-toast',
            'showcase-banner-prompt',
          ].filter((id, index, all) => id && all.indexOf(id) === index);
          for (const id of ids) {
            await banner.dismiss(id);
          }
        } catch (error) {
          console.error('Failed to dismiss banner:', error);
          page.update(d => { d.bannerError = String(error); });
        }
      },
      async toggleAutostart() {
        const autostart = lx.host.autostart;
        if (!lx.supports('host.autostart')) {
          return;
        }
        if (page.data.autostartEnabled === null) {
          await actions.refreshAutostart();
          if (page.data.autostartEnabled === null) {
            return;
          }
        }
        const next = !page.data.autostartEnabled;
        try {
          await (next ? autostart.enable() : autostart.disable());
          const enabled = await autostart.isEnabled();
          console.log('Autostart set to', next, '- OS reports', enabled);
          page.update(d => {
            d.autostartEnabled = enabled;
            d.autostartError = '';
          });
        } catch (error) {
          console.error('Failed to toggle autostart:', error);
          page.update(d => { d.autostartError = String(error); });
        }
      },
      getSystemSetting() {
        try {
          const info = lx.device.systemSettings();
          console.log('System setting:', info);
          page.update(d => { d.systemSetting = info; });
        } catch (error) {
          console.error('Failed to get system setting:', error);
          page.update(d => { d.systemSetting = null; });
        }
      },
    };
    return {
      onLoad: load,
      onShow() {
        console.log('System page onShow');
        // The user can flip the login item in System Settings / Task Manager
        // while this page is hidden 鈥?re-read the OS state on every show.
        if (page.data.currentType === 'autostart') {
          actions.refreshAutostart();
        }
        if (page.data.currentType === 'cache') {
          actions.refreshCacheSize();
        }
        if (page.data.currentType === 'notification') {
          actions.refreshNotification();
        }
        if (page.data.currentType === 'banner') {
          actions.refreshBanner();
        }
      },
      onHide() {
        console.log('System page onHide');
      },
      actions,
    };
  },
});

/** `route` is a host-registered native location, not a page. */
function notificationTargetFor(choice: string): HostTarget {
  if (choice === 'native') {
    return { route: 'showcase.native.note', params: { text: 'from a notification' } };
  }
  if (choice === 'page') {
    return { page: 'system', params: { type: 'notification' } };
  }
  return { activate: true };
}

function notificationBody(choice: string): string {
  if (choice === 'native') {
    return lx.app.t('system.notifyNative');
  }
  if (choice === 'page') {
    return lx.app.t('system.notifyPage');
  }
  return lx.app.t('system.notifyActivate');
}
