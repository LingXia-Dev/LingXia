import { definePage } from '@lingxia/types/page';
export default definePage({
  data: {},
  setup(page) {
    const actions = {
      async goBack() {
        if (page.surface) await page.surface.close();
        else await lx.nav.back();
      },
      async submitFeedback(params: { category?: string; message?: string; email?: string }) {
        const category = typeof params?.category === 'string' ? params.category.trim() : 'Other';
        const message = typeof params?.message === 'string' ? params.message.trim() : '';
        const email = typeof params?.email === 'string' ? params.email.trim() : '';
        if (!message) {
          lx.ui.toast({ title: lx.app.t('feedback.empty'), icon: 'none' });
          return;
        }
        console.log('[Feedback] Submitted', { category, message, email });
        lx.ui.toast({ title: lx.app.t('feedback.thanks'), icon: 'success' });
        await page.surface?.close();
      },
    };
    return {
      actions,
    };
  },
});
