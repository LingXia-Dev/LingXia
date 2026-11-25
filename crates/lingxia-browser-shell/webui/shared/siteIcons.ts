import { invoke } from './native';
import { originOf } from './format';

// The route takes at most this many URLs per call.
const BATCH = 400;
// A site whose icon was still being fetched is asked again after this long.
const RETRY_MS = 15_000;

/**
 * Site icons by origin, asked of the host once per site; `onFound` runs with the icons each answer
 * adds, so a page can patch only the rows they belong to.
 */
export function siteIconCache(onFound: (found: Map<string, string>) => void) {
  const icons = new Map<string, string>();
  const askedAt = new Map<string, number>();

  async function want(urls: Iterable<string>) {
    const now = Date.now();
    const ask = new Map<string, string>();
    for (const url of urls) {
      const origin = originOf(url);
      if (!origin || icons.has(origin) || ask.has(origin)) continue;
      const asked = askedAt.get(origin);
      if (asked !== undefined && now - asked < RETRY_MS) continue;
      askedAt.set(origin, now);
      ask.set(origin, url);
    }
    const urlsToAsk = [...ask.values()];
    for (let start = 0; start < urlsToAsk.length; start += BATCH) {
      const batch = urlsToAsk.slice(start, start + BATCH);
      const answer = await invoke<{ icons: Record<string, string> }>('shell.siteIcons', { urls: batch }).catch(() => null);
      if (!answer) continue;
      const found = new Map<string, string>();
      for (const [url, icon] of Object.entries(answer.icons)) {
        const origin = originOf(url);
        if (!origin) continue;
        icons.set(origin, icon);
        found.set(origin, icon);
      }
      if (found.size) onFound(found);
    }
  }

  return {
    want,
    /** The icon already known for `url`'s site. */
    iconOf(url: string): string | undefined {
      const origin = originOf(url);
      return origin ? icons.get(origin) : undefined;
    },
  };
}
