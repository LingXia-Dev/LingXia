import type { AppLifecycleEventArgs } from '@lingxia/logic';

/** State the App and its pages share. A module, so every importer sees this one object. */
export const appState = {
  lifecycle: {
    launches: 0,
    shows: 0,
    hides: 0,
    last: "",
    /** Home page `onHide` / `onShow` counts, for lifecycle probes in specs. */
    pageHides: 0,
    pageShows: 0,
    /** The last App `onShow` options, for reopen probes in specs. */
    lastShow: undefined as AppLifecycleEventArgs | undefined,
  },
  ipAddr: "loading",
};

const ipListeners = new Set<(ip: string) => void>();

/** Called with the public address once it is known. Returns an unsubscribe. */
export function watchIpAddr(listener: (ip: string) => void): () => void {
  ipListeners.add(listener);
  return () => {
    ipListeners.delete(listener);
  };
}

export function setIpAddr(ip: string): void {
  appState.ipAddr = ip;
  for (const listener of ipListeners) listener(ip);
}
