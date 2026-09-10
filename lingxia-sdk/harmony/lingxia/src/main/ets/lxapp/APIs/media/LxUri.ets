import { getCurrentLxapp, resolveLxUri, type CurrentLxApp } from 'liblingxia.so';

export function isRemoteHttpUri(input: string): boolean {
  const raw = (input ?? '').trim().toLowerCase();
  return raw.startsWith('https://') || raw.startsWith('http://');
}

export function resolveLxUriForFileAccess(input: string): string {
  const raw = (input ?? '').trim();
  if (!raw) return input;
  if (isRemoteHttpUri(raw)) return raw;

  try {
    const current: CurrentLxApp = getCurrentLxapp();
    const appid: string = current.appid;
    if (!appid) return input;
    return resolveLxUri(appid, raw) ?? input;
  } catch (_) {
    return input;
  }
}
