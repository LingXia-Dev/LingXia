import type { HostAppEnv, HostServiceEnvState } from '../src/index.js';

const state: HostServiceEnvState = lx.host.getServiceEnv();
const running: HostAppEnv = state.serviceEnv;
const next: HostAppEnv = state.nextLaunchEnv;
if (state.canToggle) {
  const result = lx.host.toggleServiceEnv();
  const saved: HostAppEnv = result.state.nextLaunchEnv;
  const current: HostAppEnv = result.state.serviceEnv;
  const requested: boolean = result.exitRequested;
  const error: string | undefined = result.exitError;
  // @ts-expect-error the target is exposed through the returned state
  result.override;
  // @ts-expect-error a successful toggle always takes effect on next launch
  result.takesEffect;
}
// @ts-expect-error persistence details are not part of the public state
state.override;
// @ts-expect-error service URLs are managed by the framework
state.lingxiaServer;
