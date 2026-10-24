Page({
  data: { count: 0 },
  increment({ step }: { step: number }) {
    if (!Number.isInteger(step) || step < 1 || step > 10) throw new Error('Invalid greeting step');
    this.setData({ count: this.data.count + step });
    return this.data.count;
  },
  async setAppearance({ preference }: { preference: 'auto' | 'light' | 'dark' }) {
    if (!['auto', 'light', 'dark'].includes(preference)) throw new Error('Invalid appearance');
    const control = lx.host.control;
    if (!control) throw new Error('Appearance settings require the control app');
    await control.appearance.setPreference(preference);
  },
  async openClipboard() { await lx.navigateTo({ page: "clipboard" }); },
  async openShare() { await lx.navigateTo({ page: "share" }); },
  async openTodo() { await lx.navigateTo({ page: "todo" }); },
  resetGreetings() {
    this.setData({ count: 0 });
  },
});
