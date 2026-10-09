import type { Fixture } from '@lingxia/test';

export interface AndroidNode {
  text: string;
  description: string;
  resource: string;
  bounds: [number, number, number, number];
}

export function androidDevice(t: Fixture) {
  const base = t.arg('androidDevice');
  if (!base) throw new Error('Android device fixture is required');
  const send = async <T>(path: string, body?: object): Promise<T> => {
    const response = await fetch(`${base}/${path}`, body === undefined ? {} : {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(`Android ${path}: ${response.status} ${await response.text()}`);
    return response.json() as Promise<T>;
  };
  // A timed-out assertion can leave a UI dump in flight while cleanup begins.
  let pending: Promise<unknown> = Promise.resolve();
  const request = <T>(path: string, body?: object): Promise<T> => {
    const result = pending.then(() => send<T>(path, body));
    pending = result.catch(() => undefined);
    return result;
  };
  const nodes = async (): Promise<AndroidNode[]> => {
    const { xml } = await request<{ xml: string }>('hierarchy');
    return [...xml.matchAll(/<node\b[^>]*>/g)].map(([node]) => {
      const attr = (name: string) => (node.match(new RegExp(` ${name}="([^"]*)"`))?.[1] ?? '')
        .replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
      const coordinates = attr('bounds').match(/-?\d+/g)?.map(Number) ?? [];
      if (coordinates.length !== 4) throw new Error('Invalid Android node bounds');
      return { text: attr('text'), description: attr('content-desc'), resource: attr('resource-id'), bounds: coordinates as AndroidNode['bounds'] };
    });
  };
  const tap = async (node: AndroidNode) => {
    const [left, top, right, bottom] = node.bounds;
    if (left < 0 || top < 0 || right <= left || bottom <= top) throw new Error('Android control has no tappable bounds');
    await request('tap', { x: Math.round((left + right) / 2), y: Math.round((top + bottom) / 2) });
  };
  return {
    nodes, tap,
    key: (key: 'back' | 'enter' | 'tab' | 'escape') => request('key', { key }),
    size: () => request<{ width: number; height: number }>('size'),
    swipe: (input: { x1: number; y1: number; x2: number; y2: number; duration: number }) => request('swipe', input),
    async tapText(text: string) {
      const found = await t.waitFor(nodes, { until: (items) => items.some((item) => item.text === text), timeout: 15000 });
      const matches = found.filter((item) => item.text === text);
      if (matches.length !== 1) throw new Error(`Expected one Android control '${text}', found ${matches.length}`);
      await tap(matches[0]);
    },
  };
}
