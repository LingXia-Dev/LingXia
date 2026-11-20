import { appState, setIpAddr } from './shared/lib/app';
import { openFeedback } from './shared/lib/feedback';

App({
  // For `lxdev test`, which reads it through `appInstance().state`.
  state: appState,
  onLaunch() {
    appState.lifecycle.launches++;
    appState.lifecycle.last = 'launch';
    lx.app.setMoreActions([{
      icon: 'public/showcase-icon.png',
      label: 'Feedback',
      onClick: async () => {
        try { await openFeedback(); }
        catch (error) { console.warn('failed to open feedback surface', error); }
      },
    }]);
    // Not awaited: the first page's onLoad waits for onLaunch.
    void loadIpAddr();
  },
  onShow() {
    appState.lifecycle.shows++;
    appState.lifecycle.last = 'show';
  },
  onHide() {
    appState.lifecycle.hides++;
    appState.lifecycle.last = 'hide';
  },
});

async function loadIpAddr(): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10_000);
  try {
    const response = await fetch('https://api64.ipify.org?format=json', { signal: controller.signal });
    if (!response.ok) throw new Error('Network information unavailable');
    const data = await response.json() as { ip: string };
    setIpAddr(data.ip);
  } catch {
    setIpAddr('Network information unavailable');
  } finally {
    clearTimeout(timeout);
  }
}
