import { definePage, lx } from '@lingxia/logic';

export default definePage({
  setup: page => ({
    actions: {
      async submitFeedback({ category, message, email }: { category: string; message: string; email: string }) {
        if (!message.trim()) {
          lx.ui.toast('Tell us what happened');
          return;
        }
        console.log('[Feedback] Submitted', { category, message: message.trim(), email: email.trim() });
        lx.ui.toast('Thanks for the feedback', { tone: 'success' });
        // Opened as a float, the page closes its own surface; pushed, it stays.
        await page.surface?.close();
      },
    },
  }),
});
