Page({
  // Stands in for a real Logic action, so the AsyncAction example shows its loading state.
  async publish() {
    await new Promise(resolve => setTimeout(resolve, 1200));
  },
});
