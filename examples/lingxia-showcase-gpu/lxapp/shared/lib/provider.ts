import type { CloudApi, CloudAuthApi } from "../../provider";

// `lx.cloud` / `lx.auth` exist only when the build injected the cloud provider
// (`lingxia build --with-provider cloud`), so the page has to prove presence
// before it calls one. Reading them through here keeps that check in one place
// and turns absence into a message instead of a TypeError.
export function cloudProviderAvailable(): boolean {
  return Boolean(lx.cloud && lx.auth);
}

export function cloudApi(): CloudApi {
  if (!lx.cloud) throw new Error(lx.app.t('cloud.noProvider'));
  return lx.cloud;
}

export function authApi(): CloudAuthApi {
  if (!lx.auth) throw new Error(lx.app.t('cloud.noProvider'));
  return lx.auth;
}
