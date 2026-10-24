import type { ShowcaseAppInstance } from './shared/lib/app';
import { openFeedback } from './shared/lib/feedback';

App({
  globalData: {
    lifecycle: { launches: 0, shows: 0, hides: 0, last: '' },
    greeting: "This is from App's globalData.data",
    ipAddr: 'loading',
  },
  async onLaunch(this: ShowcaseAppInstance) {
    this.globalData.lifecycle.launches++;
    this.globalData.lifecycle.last = 'launch';
    lx.setMoreActions([{
      icon: 'public/showcase-icon.png',
      label: 'Feedback',
      onClick: async () => {
        try { await openFeedback(); }
        catch (error) { console.warn('failed to open feedback surface', error); }
      },
    }]);
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 10_000);
    try {
      const response = await fetch('https://api64.ipify.org?format=json', { signal: controller.signal });
      if (!response.ok) throw new Error('Network information unavailable');
      const data = await response.json() as { ip: string };
      this.globalData.ipAddr = data.ip;
    } catch {
      this.globalData.ipAddr = 'Network information unavailable';
    } finally {
      clearTimeout(timeout);
    }
    this.ipReadyCallback?.(this.globalData.ipAddr);
  },
  onShow(this: ShowcaseAppInstance) {
    this.globalData.lifecycle.shows++;
    this.globalData.lifecycle.last = 'show';
  },
  onHide(this: ShowcaseAppInstance) {
    this.globalData.lifecycle.hides++;
    this.globalData.lifecycle.last = 'hide';
  },
});
