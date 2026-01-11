import { getCurrentLxapp, resolveLxUri, type CurrentLxApp } from 'liblingxia.so';

export function resolveLxUriForFileAccess(input: string): string {
  const raw = (input ?? '').trim();
  if (!raw) return input;
  if (raw.startsWith('http://') || raw.startsWith('https://')) return raw;

  try {
    const current: CurrentLxApp = getCurrentLxapp();
    const appid: string = current.appid;
    if (!appid) return input;
    return resolveLxUri(appid, raw) ?? input;
  } catch (_) {
    return input;
  }
}
