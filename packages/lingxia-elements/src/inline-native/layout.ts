import { OBSERVED_STYLE_FIELDS } from "./style.js";

const NODE_SELECTOR = "lx-native-view,lx-native-cover,lx-native-text,lx-native-button,lx-video";

export type NativeLayoutChange = "geometry" | "style";
type Measurement = { geometry: string; style: string };
type Subscription = {
  root: HTMLElement;
  invalidate: (change: NativeLayoutChange) => void;
  elements: Element[];
  geometry: string;
  style: string;
};
const subscriptions = new Set<Subscription>();
let frame: number | null = null;
let observer: MutationObserver | undefined;
let stylesDirty = false;
let membersDirty = false;

function members(root: HTMLElement): Element[] {
  const elements: Element[] = [root, ...Array.from(root.querySelectorAll(NODE_SELECTOR))];
  for (let ancestor = root.parentElement; ancestor; ancestor = ancestor.parentElement) elements.push(ancestor);
  return elements;
}

function sample(): void {
  frame = null;
  // Roots often share the same ancestor chain. Read each element once per
  // sample, while retaining polling for CSSOM and pseudo-class paint changes.
  const measured = new Map<Element, Measurement>();
  const invalidations: Array<() => void> = [];
  for (const entry of subscriptions) {
    if (!entry.root.isConnected) continue;
    if (membersDirty) entry.elements = members(entry.root);
    const snapshots = entry.elements
      .map((element) => {
        const cached = measured.get(element);
        if (cached) return cached;
        const rect = element.getBoundingClientRect();
        const style = getComputedStyle(element);
        // CSSOM, pseudo-classes and media queries can change paint without a DOM mutation.
        const paint = OBSERVED_STYLE_FIELDS.map(field => style[field as keyof CSSStyleDeclaration]).join(",");
        const snapshot = {
          geometry: `${rect.x},${rect.y},${rect.width},${rect.height},${element.clientWidth},${element.clientHeight}`,
          style: paint,
        };
        measured.set(element, snapshot);
        return snapshot;
      });
    const geometry = snapshots.map(snapshot => snapshot.geometry).join(";");
    const style = snapshots.map(snapshot => snapshot.style).join(";");
    const change = stylesDirty || style !== entry.style ? "style"
      : geometry !== entry.geometry ? "geometry" : undefined;
    entry.geometry = geometry;
    entry.style = style;
    if (change) invalidations.push(() => entry.invalidate(change));
  }
  stylesDirty = false;
  membersDirty = false;
  // Keep callbacks from writing layout between two roots' measurements.
  for (const invalidate of invalidations) invalidate();
  if (subscriptions.size) frame = requestAnimationFrame(sample);
}

/** ResizeObserver misses position-only changes (siblings, fonts and ancestor layout). */
export function observeNativeLayout(root: HTMLElement, invalidate: (change: NativeLayoutChange) => void): () => void {
  const entry: Subscription = { root, invalidate, elements: members(root), geometry: "", style: "" };
  subscriptions.add(entry);
  if (!observer) {
    observer = new MutationObserver((records) => {
      if (records.some(record => record.type === "childList")) membersDirty = true;
      // Root already observes its own subtree. External changes may alter inherited styles.
      const roots = Array.from(subscriptions, ({ root }) => root);
      if (records.some((record) => !roots.some(root => root.contains(record.target)))) {
        stylesDirty = true;
      }
    });
    observer.observe(document.documentElement, { attributes: true, childList: true, characterData: true, subtree: true });
  }
  if (frame === null) frame = requestAnimationFrame(sample);
  return () => {
    subscriptions.delete(entry);
    if (!subscriptions.size) {
      observer?.disconnect();
      observer = undefined;
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      stylesDirty = false;
      membersDirty = false;
    }
  };
}
