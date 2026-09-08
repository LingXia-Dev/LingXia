const NODE_SELECTOR = "lx-native-view,lx-native-cover,lx-native-text,lx-native-button,lx-video";

type Subscription = { root: HTMLElement; invalidate: () => void; geometry: string };
const subscriptions = new Set<Subscription>();
let frame: number | null = null;
let observer: MutationObserver | undefined;
let stylesDirty = false;

function sample(): void {
  frame = null;
  for (const entry of subscriptions) {
    if (!entry.root.isConnected) continue;
    const geometry = [entry.root, ...Array.from(entry.root.querySelectorAll(NODE_SELECTOR))]
      .map((element) => {
        const rect = element.getBoundingClientRect();
        return `${rect.x},${rect.y},${rect.width},${rect.height}`;
      }).join(";");
    if (stylesDirty || geometry !== entry.geometry) {
      entry.geometry = geometry;
      entry.invalidate();
    }
  }
  stylesDirty = false;
  if (subscriptions.size) frame = requestAnimationFrame(sample);
}

/** ResizeObserver misses position-only changes (siblings, fonts and ancestor layout). */
export function observeNativeLayout(root: HTMLElement, invalidate: () => void): () => void {
  const entry = { root, invalidate, geometry: "" };
  subscriptions.add(entry);
  if (!observer) {
    observer = new MutationObserver((records) => {
      // Root already observes its own subtree. External changes may alter inherited styles.
      if (records.some((record) => !Array.from(subscriptions).some(({ root }) => root.contains(record.target)))) {
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
    }
  };
}
