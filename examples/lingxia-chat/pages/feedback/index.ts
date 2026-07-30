Page({
  data: {},

  submitFeedback: async function (params) {
    const category = typeof params?.category === 'string' ? params.category.trim() : 'Other';
    const message = typeof params?.message === 'string' ? params.message.trim() : '';
    const email = typeof params?.email === 'string' ? params.email.trim() : '';

    if (!message) {
      lx.showToast({ title: 'Tell us what happened', icon: 'none' });
      return;
    }

    console.log('[Feedback] Submitted', { category, message, email });
    lx.showToast({ title: 'Thanks for the feedback', icon: 'success' });
    await this.surface?.close();
  },
});
