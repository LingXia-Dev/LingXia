import { cloudProviderAvailable } from '../../shared/lib/provider';
import { errorMessage } from '../../shared/lib/errors';
import { openFeedback } from '../../shared/lib/feedback';
import { definePage, lx } from '@lingxia/logic';

export default definePage({
  data: { cloudAvailable: false, loadCount: 0 },
  setup(page) {
    const actions = {
      async openOtherApp() {
        try { await lx.nav.go({ app: 'lingxia-showcase-web', page: 'chat' }); }
        catch (error) { await lx.ui.toast(errorMessage(error, 'Unable to open the app. Please try again.')); }
      },
      async openDeepSeek() {
        const choice = await lx.ui.actionSheet({
          items: [{ id: 'self', label: 'This window' }, { id: 'aside', label: 'Side panel' }, { id: 'external', label: 'Default browser' }],
        });
        if (choice.status === 'canceled') return;
        try {
          const url = 'https://www.deepseek.com/';
          if (choice.id === 'external') await lx.openExternal(url);
          else if (choice.id === 'aside') await lx.surface.openUrl(url, { as: 'aside' });
          else await lx.surface.openUrl(url);
        } catch (error) {
          await lx.ui.toast(errorMessage(error, 'Unable to open the page. Please try again.'));
        }
      },
      async exitApp() {
        const exit = await lx.ui.confirm({ title: 'Exit app', message: 'Close the app now?', tone: 'danger', confirmText: 'Exit', cancelText: 'Cancel' });
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
