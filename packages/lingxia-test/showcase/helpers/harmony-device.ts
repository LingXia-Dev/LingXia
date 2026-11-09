import type { Fixture } from '@lingxia/test';

export interface HarmonyNode {
  text: string;
  id: string;
  type: string;
  /** `[left, top, right, bottom]` in screen pixels. */
  bounds: [number, number, number, number];
}

export type HarmonyKey = 'back' | 'enter' | 'tab' | 'escape' | 'up' | 'down' | 'home' | 'end'
  | 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g' | 'h' | 'i' | 'j' | 'k' | 'l' | 'm'
  | 'n' | 'o' | 'p' | 'q' | 'r' | 's' | 't' | 'u' | 'v' | 'w' | 'x' | 'y' | 'z';

export function harmonyDevice(t: Fixture) {
  const base = t.arg('harmonyDevice');
  if (!base) throw new Error('Harmony device fixture is required');
  const request = async <T>(path: string, body?: object): Promise<T> => {
    const response = await fetch(`${base}/${path}`, body === undefined ? {} : {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(`Harmony ${path}: ${response.status} ${await response.text()}`);
    return response.json() as Promise<T>;
  };
  const nodes = async (): Promise<HarmonyNode[]> => (await request<{ nodes: HarmonyNode[] }>('layout')).nodes;
  return {
    nodes,
    /** Colours of the composited screen (every window and surface) at screen pixels, as `[r, g, b]`. */
    pixels: async (points: readonly (readonly [number, number])[]) =>
      (await request<{ pixels: [number, number, number][] }>('pixels', { points })).pixels,
    swipe: (input: { x1: number; y1: number; x2: number; y2: number; velocity: number }) => request('swipe', input),
    /** A hardware key press; the Android fixture's names plus arrows, Home, End and letters. */
    key: (key: HarmonyKey) => request('key', { key }),
    /** A finger tap at screen pixels, through the system input path. */
    click: (x: number, y: number) => request('click', { x, y }),
    /** The system back gesture: a finger from the left edge to the middle of the screen. */
    async edgeBack() {
      const screen = (await nodes()).reduce((a, b) => (b.bounds[2] * b.bounds[3] > a.bounds[2] * a.bounds[3] ? b : a));
      const [, , width, height] = screen.bounds;
      const y = Math.round(height / 2);
      await request('swipe', { x1: 2, y1: y, x2: Math.round(width / 2), y2: y, velocity: 1500 });
    },
  };
}
