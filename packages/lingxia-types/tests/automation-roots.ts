import type { Automation, HostRunAutomation, LogicLxAppDriver, LxAppDriver } from '../src/automation/index.js';

// What `import { lx } from '@lingxia/logic'` gives.
declare const lx: import('../src/index.js').Lx;

// App Logic: `lx.automation()` is the Logic-facing root.
async function logicRoot() {
  const logic: Automation = lx.automation();
  const self: LogicLxAppDriver = logic.lxapp();
  await self.nav.go({ page: 'home' });
  await self.nav.back();
  await self.nav.back(2, { waitUntil: 'commit' });
  // @ts-expect-error the page count is the first argument, not an option
  await self.nav.back({ delta: 2 });
  await logic.lxapp('other').eval<number>({ script: '1' });
  // @ts-expect-error test network routing exists only in a host automation run
  void self.network;
  // @ts-expect-error app Logic cannot await its own page's onReady
  await self.nav.go({ page: 'home' }, { waitUntil: 'ready' });
  // @ts-expect-error timeoutMs bounds only a ready wait
  await self.nav.reset({ page: 'home' }, { timeoutMs: 1000 });
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
  await app.nav.go({ page: 'home' }, { waitUntil: 'ready', timeoutMs: 5000 });
  await app.network.unrouteAll();
  // A host-run driver still satisfies code written against the Logic shape.
  const asLogic: LogicLxAppDriver = run.lxapp('other');
  const asRoot: Automation = run;
  void [asLogic, asRoot];
}

void [logicRoot, hostRunRoot];
