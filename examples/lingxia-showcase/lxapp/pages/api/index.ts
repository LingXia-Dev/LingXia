import { cloudProviderAvailable } from '../../shared/lib/provider';
import { failureText } from '../../shared/lib/errors';
import { openFeedback } from '../../shared/lib/feedback';
import { definePage, lx } from '@lingxia/logic';

export default definePage({
  data: { cloudAvailable: false, loadCount: 0 },
  setup(page) {
    const actions = {
      async openOtherApp() {
        try { await lx.nav.go({ app: 'lingxia-showcase-web', page: 'chat' }); }
        catch (error) { await lx.ui.toast(failureText(error, lx.app.t('api.openAppFailed'))); }
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
          await lx.ui.toast(failureText(error, lx.app.t('api.openPageFailed')));
        }
      },
      async exitApp() {
        const exit = await lx.ui.confirm({ title: lx.app.t('api.exit'), message: lx.app.t('api.exitConfirm'), tone: 'danger', confirmText: lx.app.t('api.exitAction'), cancelText: lx.app.t('common.cancel') });
        if (exit) await lx.host.exit();
      },
      async openFeedback() { await openFeedback(); },
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
