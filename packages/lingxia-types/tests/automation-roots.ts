import type { Automation, HostRunAutomation, LogicLxAppDriver, LxAppDriver } from '../src/automation/index.js';

// App Logic: `lx.automation()` is the Logic-facing root.
async function logicRoot() {
  const logic: Automation = lx.automation();
  const self: LogicLxAppDriver = logic.lxapp();
  await self.nav.to({ page: 'home' });
  await self.nav.back({ waitUntil: 'commit' });
  await logic.lxapp('other').eval<number>({ script: '1' });
  // @ts-expect-error test network routing exists only in a host automation run
  void self.network;
  // @ts-expect-error app Logic cannot await its own page's onReady
  await self.nav.to({ page: 'home', waitUntil: 'ready' });
  // @ts-expect-error timeoutMs bounds only a ready wait
  await self.nav.relaunch({ page: 'home', timeoutMs: 1000 });
  // @ts-expect-error call tracing is test-runner plumbing
  await self.eval({ script: '1', captureCalls: true });
  // The host tiers stay on the Logic root behind the `host` privilege.
  void logic.lxapps;
  void logic.desktop;
}

// Host automation run: the same root plus the test-run-only members.
declare const run: HostRunAutomation;
async function hostRunRoot() {
  const app: LxAppDriver = run.lxapp();
  await app.nav.to({ page: 'home', waitUntil: 'ready', timeoutMs: 5000 });
  await app.network.unrouteAll();
  // A host-run driver still satisfies code written against the Logic shape.
  const asLogic: LogicLxAppDriver = run.lxapp('other');
  const asRoot: Automation = run;
  void [asLogic, asRoot];
}

void [logicRoot, hostRunRoot];
