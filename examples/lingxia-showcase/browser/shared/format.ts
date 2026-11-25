export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let idx = 0;
  let value = bytes;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx += 1;
  }
  return idx === 0 ? `${Math.round(value)} ${units[idx]}` : `${value.toFixed(1)} ${units[idx]}`;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** `ms` as the user's local `YYYY-MM-DDTHH:mm`, the form Liuli's Timestamp reads. */
export function localDateTime(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** The URL's host without `www.`, or the URL itself when it has none. */
export function hostOf(url: string): string {
  const match = /^[a-z][a-z0-9+.-]*:\/\/([^/?#:]+)/i.exec(url);
  return match ? match[1].replace(/^www\./i, '') : url;
}

/** `scheme://host[:port]` of an http(s) URL, the key site icons are cached by; null for other URLs. */
export function originOf(url: string): string | null {
  const match = /^(https?:\/\/[^/?#]+)/i.exec(url.trim());
  return match ? match[1].toLowerCase() : null;
}
