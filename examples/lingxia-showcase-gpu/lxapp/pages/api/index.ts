import { cloudProviderAvailable } from '../../shared/lib/provider';
import { errorMessage } from '../../shared/lib/errors';
import { openFeedback } from '../../shared/lib/feedback';

Page({
  data: { cloudAvailable: false },
  onLoad() { this.setData({ cloudAvailable: cloudProviderAvailable() }); },
  async openOtherApp() {
    try { await lx.navigateToApp({ appId: 'lingxia-chat', page: 'chat' }); }
    catch (error) { await lx.showToast({ title: errorMessage(error, 'Unable to open the app. Please try again.'), icon: 'none' }); }
  },
  async openDeepSeek() {
    const choice = await lx.showActionSheet({
      items: [{ id: 'self', label: 'This window' }, { id: 'aside', label: 'Side panel' }, { id: 'external', label: 'Default browser' }],
      itemColor: '#007AFF',
    });
    if (choice.status === 'canceled') return;
    try {
      const url = 'https://www.deepseek.com/';
      if (choice.id === 'external') await lx.openExternal(url);
      else if (choice.id === 'aside') await lx.surface.openUrl(url, { as: 'aside' });
      else await lx.surface.openUrl(url);
    } catch (error) {
      await lx.showToast({ title: errorMessage(error, 'Unable to open the page. Please try again.'), icon: 'none' });
    }
  },
  async exitApp() {
    const result = await lx.showModal({ title: 'Exit app', content: 'Close the app now?', confirmText: 'Exit', cancelText: 'Cancel' });
    if (result.status !== 'canceled') await lx.host.exit();
  },
  async goBack() { await lx.navigateBack(); },
  async openDevice({ type = 'device' }: { type?: string } = {}) {
    await lx.navigateTo({ page: 'device', query: { type } });
  },
  async openSystem({ type = 'appBaseInfo' }: { type?: string } = {}) {
    await lx.navigateTo({ page: 'system', query: { type } });
  },
  async openClipboard() { await lx.navigateTo({ page: 'clipboard' }); },
  async openShare() { await lx.navigateTo({ page: 'share' }); },
  async openWifi() { await lx.navigateTo({ page: 'wifi' }); },
  async openLocation() { await lx.navigateTo({ page: 'location' }); },
  async openFile({ section = 'openFile' }: { section?: 'openFile' | 'chooseFile' } = {}) {
    await lx.navigateTo({ page: 'file', query: { section } });
  },
  async openStream() { await lx.navigateTo({ page: 'stream' }); },
  async openRefresh() { await lx.navigateTo({ page: 'pulltorefresh' }); },
  async openLeaveGuard() { await lx.navigateTo({ page: 'leaveguard' }); },
  async openFeedback() { await openFeedback(); },
  async openCloud({ type = 'auth' }: { type?: string } = {}) {
    await lx.navigateTo({ page: 'cloud', query: { type } });
  },
  async openMedia({ type = 'Pictures' }: { type?: string } = {}) {
    await lx.navigateTo({ page: 'media', query: { type } });
  },
  async openChannel() { await lx.navigateTo({ page: 'channel' }); },
  async openBridge() { await lx.navigateTo({ page: 'bridge-repro' }); },
  async openUi({ type = 'navigation' }: { type?: string } = {}) {
    await lx.navigateTo({ page: 'ui', query: { type } });
  },
});
