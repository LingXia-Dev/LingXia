import type { HostAppEnv, HostServiceEnvState } from '../src/index.js';

// What `import { lx } from '@lingxia/logic'` gives.
declare const lx: import('../src/index.js').Lx;

const state: HostServiceEnvState = lx.host.serviceEnv.get();
const running: HostAppEnv = state.serviceEnv;
const next: HostAppEnv = state.nextLaunchEnv;
if (state.canSwitch) {
  const result = lx.host.serviceEnv.set(state.serviceEnv === 'prod' ? 'dev' : 'prod');
  const saved: HostAppEnv = result.state.nextLaunchEnv;
  const current: HostAppEnv = result.state.serviceEnv;
  const requested: boolean = result.exitRequested;
  const error: string | undefined = result.exitError;
  // @ts-expect-error the target is exposed through the returned state
  result.override;
  // @ts-expect-error a successful switch always takes effect on next launch
  result.takesEffect;
  // @ts-expect-error the target environment is named
  lx.host.serviceEnv.set();
  // @ts-expect-error only a host env is a target
  lx.host.serviceEnv.set('staging');
}
// @ts-expect-error persistence details are not part of the public state
state.override;
// @ts-expect-error service URLs are managed by the framework
state.lingxiaServer;
