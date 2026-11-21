import { cloudProviderAvailable } from '../../shared/lib/provider';
import { errorMessage } from '../../shared/lib/errors';
import { openFeedback } from '../../shared/lib/feedback';
import { definePage } from '@lingxia/types/page';

export default definePage({
  data: { cloudAvailable: false, loadCount: 0 },
  setup(page) {
    const actions = {
      async openOtherApp() {
        try { await lx.nav.go({ app: 'lingxia-chat', page: 'chat' }); }
        catch (error) { await lx.ui.toast({ title: errorMessage(error, lx.app.t('api.openAppFailed')), icon: 'none' }); }
      },
      async openDeepSeek() {
        const choice = await lx.ui.actionSheet({
          items: [{ id: 'self', label: lx.app.t('api.openIn.self') }, { id: 'aside', label: lx.app.t('api.openIn.aside') }, { id: 'external', label: lx.app.t('api.openIn.external') }],
        });
        if (choice.status === 'canceled') return;
        try {
          const url = 'https://www.deepseek.com/';
          if (choice.id === 'external') await lx.openExternal(url);
          else if (choice.id === 'aside') await lx.surface.openUrl(url, { as: 'aside' });
          else await lx.surface.openUrl(url);
        } catch (error) {
          await lx.ui.toast({ title: errorMessage(error, lx.app.t('api.openPageFailed')), icon: 'none' });
        }
      },
      async exitApp() {
        const exit = await lx.ui.confirm({ title: lx.app.t('api.exit'), message: lx.app.t('api.exitConfirm'), tone: 'danger', confirmText: lx.app.t('api.exitAction'), cancelText: lx.app.t('common.cancel') });
        if (exit) await lx.host.exit();
      },
      async goBack() { await lx.nav.back(); },
      async openDevice({ type = 'device' }: { type?: string } = {}) {
        await lx.nav.go({ page: 'device', params: { type } }).opened;
      },
      async openSystem({ type = 'appBaseInfo' }: { type?: string } = {}) {
        await lx.nav.go({ page: 'system', params: { type } }).opened;
      },
      async openClipboard() { await lx.nav.go({ page: 'clipboard' }).opened; },
      async openShare() { await lx.nav.go({ page: 'share' }).opened; },
      async openWifi() { await lx.nav.go({ page: 'wifi' }).opened; },
      async openLocation() { await lx.nav.go({ page: 'location' }).opened; },
      async openFile({ section = 'openFile' }: { section?: 'openFile' | 'chooseFile' } = {}) {
        await lx.nav.go({ page: 'file', params: { section } }).opened;
      },
      async openStream() { await lx.nav.go({ page: 'stream' }).opened; },
      async openRefresh() { await lx.nav.go({ page: 'pullToRefresh' }).opened; },
      async openLeaveGuard() { await lx.nav.go({ page: 'leaveGuard' }).opened; },
      async openFeedback() { await openFeedback(); },
      async openCloud({ type = 'auth' }: { type?: string } = {}) {
        await lx.nav.go({ page: 'cloud', params: { type } }).opened;
      },
      async openMedia({ type = 'Pictures' }: { type?: string } = {}) {
        await lx.nav.go({ page: 'media', params: { type } }).opened;
      },
      async openChannel() { await lx.nav.go({ page: 'channel' }).opened; },
      async openBridge() { await lx.nav.go({ page: 'bridge-repro' }).opened; },
      async openUi({ type = 'navigation' }: { type?: string } = {}) {
        await lx.nav.go({ page: 'ui', params: { type } }).opened;
      },
    };
    return {
      onLoad() {
        page.update(d => {
          d.cloudAvailable = cloudProviderAvailable();
          d.loadCount = (page.data.loadCount || 0) + 1;
        });
      },
      actions,
    };
  },
});
