import { contract, eventually } from '../../support/contract.js';

contract(
  {
    id: 'WINDOWS-NAV-001',
    title: 'leave no native overlay window after page navigation',
    covers: ['NavDriver.relaunch', 'NavDriver.to', 'NavDriver.back', 'DesktopDriver.windows'],
    layer: 'native',
    levels: ['boundary', 'lifecycle'],
    scope: 'windows',
    expectedOutcome: 'supported',
  },
  async ({ app }) => {
    const desktop = lx.automation().desktop;
    const host = (await desktop.windows()).find((window) => (
      window.visible
      && window.process.toLocaleLowerCase().includes('lingxiademo')
      && window.title === 'LingXia'
    ));
    if (!host) throw new Error('Could not identify the Windows showcase host window');

    const visibleBefore = new Set((await desktop.windows())
      .filter((window) => window.visible && window.pid === host.pid)
      .map((window) => window.id));

    await app.nav.relaunch({ page: 'home' });
    await app.nav.to({ page: 'device', query: { type: 'screen' } });
    await app.nav.back();
    await app.nav.to({ page: 'components' });
    await app.nav.redirect({ page: 'picker' });
    await app.nav.relaunch({ page: 'device' });

    await eventually(
      async () => (await desktop.windows()).filter((window) => (
        window.visible
        && window.pid === host.pid
        && window.title === ''
        && !visibleBefore.has(window.id)
      )),
      (windows) => windows.length === 0,
      {
        describe: 'navigation-created native overlays to close',
        timeoutMs: 3_000,
      },
    );
  },
);
