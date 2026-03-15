export function getPropOrAttr(el: HTMLElement, name: string): unknown {
  const attr = el.getAttribute(name);
  if (attr !== null) return attr;
  const camelName = name.replace(/-([a-z])/g, (_m, ch: string) => ch.toUpperCase());
  const self = el as unknown as Record<string, unknown>;
  if (Object.prototype.hasOwnProperty.call(self, camelName)) {
    const value = self[camelName];
    if (value !== undefined && value !== null) return value;
  }
  return undefined;
}

export function parseNumberLike(value: unknown): number | undefined {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : undefined;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

export function getBoolAttr(el: HTMLElement, name: string): boolean {
  const raw = getPropOrAttr(el, name);
  if (raw === undefined || raw === null) return false;
  if (typeof raw === "boolean") return raw;
  const val = String(raw).trim().toLowerCase();
  if (val === "false" || val === "0" || val === "null" || val === "undefined") {
    return false;
  }
  return true;
}

export function getNumAttr(el: HTMLElement, name: string): number | undefined {
  const raw = getPropOrAttr(el, name);
  if (raw === undefined || raw === null) return undefined;
  if (typeof raw === "number") return Number.isFinite(raw) ? raw : undefined;
  const val = String(raw).trim();
  if (val === "") return undefined;
  const n = Number(val);
  return Number.isNaN(n) ? undefined : n;
}

export function normalizeBindingEventKey(rawKey: string): string | null {
  const key = rawKey.trim().toLowerCase().replace(/[\-_:]/g, "");
  return key.length > 0 ? key : null;
}

export function extractBindingEventKey(attrName: string): string | null {
  const attr = attrName.trim().toLowerCase();
  let suffix: string | null = null;
  if (attr.startsWith("bind") && attr.length > 4) {
    suffix = attr.slice(4);
  } else if (attr.startsWith("catch") && attr.length > 5) {
    suffix = attr.slice(5);
  }
  if (!suffix) return null;
  return normalizeBindingEventKey(suffix.replace(/^[:\-]/, ""));
}

export function shouldRefreshForBindingAttribute(name: string): boolean {
  const normalized = name.trim().toLowerCase();
  return normalized.startsWith("bind") || normalized.startsWith("catch") || normalized.startsWith("data-");
}

export function collectPageFuncBindings(el: HTMLElement): Record<string, string> | undefined {
  const bindings: Record<string, string> = {};
  const attrs = el.getAttributeNames();
  for (const attr of attrs) {
    const eventKey = extractBindingEventKey(attr);
    if (!eventKey) continue;
    const funcName = el.getAttribute(attr)?.trim();
    if (!funcName) continue;
    bindings[eventKey] = funcName;
  }
  return Object.keys(bindings).length > 0 ? bindings : undefined;
}

function dataAttrToDatasetKey(attr: string): string {
  const raw = attr.slice(5).trim();
  if (!raw) return "";
  const parts = raw.split("-").filter(Boolean);
  if (parts.length === 0) return "";
  return parts
    .map((segment, index) => {
      if (index === 0) return segment.toLowerCase();
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    })
    .join("");
}

export function collectDataset(el: HTMLElement): Record<string, string> {
  const dataset: Record<string, string> = {};
  const attrs = el.getAttributeNames();
  for (const attr of attrs) {
    if (!attr.startsWith("data-")) continue;
    const key = dataAttrToDatasetKey(attr);
    if (!key) continue;
    const value = el.getAttribute(attr);
    if (value == null) continue;
    dataset[key] = value;
  }
  return dataset;
}

export function buildPageFuncEvent(
  componentId: string,
  eventName: string,
  detail: unknown,
  dataset: Record<string, string>
): Record<string, unknown> {
  const target = {
    id: componentId,
    dataset
  };
  return {
    type: eventName,
    detail: detail ?? {},
    target,
    currentTarget: target,
    timeStamp: Date.now()
  };
}

export function dispatchLogicBinding(
  el: HTMLElement,
  componentId: string,
  eventName: string,
  detail: unknown,
  bindings: Record<string, string> | undefined
): void {
  const normalizedEvent = normalizeBindingEventKey(eventName);
  if (!normalizedEvent) return;
  const functionName = bindings?.[normalizedEvent];
  if (!functionName) return;

  const bridge = (window as Window & {
    LingXiaBridge?: { notify?: (method: string, params?: unknown) => void };
  }).LingXiaBridge;
  if (!bridge || typeof bridge.notify !== "function") return;

  const payload = buildPageFuncEvent(
    componentId,
    normalizedEvent,
    detail,
    collectDataset(el)
  );
  try {
    bridge.notify(functionName, payload);
  } catch {
    // Ignore notify errors.
  }
}

function findScrollableAncestor(el: HTMLElement): HTMLElement | null {
  let node: HTMLElement | null = el.parentElement;
  while (node) {
    const style = getComputedStyle(node);
    const overflowY = style.overflowY;
    const scrollable = (overflowY === "auto" || overflowY === "scroll") && node.scrollHeight > node.clientHeight + 1;
    if (scrollable) {
      return node;
    }
    node = node.parentElement;
  }
  return null;
}

function applyScrollDelta(el: HTMLElement, deltaY: number): void {
  if (Math.abs(deltaY) <= 1) return;
  const target = findScrollableAncestor(el);
  if (target) {
    target.scrollTop += Math.round(deltaY);
    return;
  }
  window.scrollBy(0, Math.round(deltaY));
}

export function ensureElementVisibleForKeyboard(
  el: HTMLElement,
  explicitKeyboardHeight = 0,
  forceCenter = false,
  extraDelays: number[] = [120]
): void {
  const tryScroll = () => {
    if (!el.isConnected) return;
    const rect = el.getBoundingClientRect();
    const layoutViewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
    const visualViewport = window.visualViewport;
    const visualBottom = visualViewport ? (visualViewport.offsetTop + visualViewport.height) : layoutViewportHeight;
    const keyboardHeight = Math.max(0, explicitKeyboardHeight || 0);
    const keyboardLikelyVisible =
      keyboardHeight > 0 ||
      (!!visualViewport && visualViewport.height < (layoutViewportHeight - 40));
    if (!keyboardLikelyVisible) {
      return;
    }
    const viewportHeight = keyboardHeight > 0
      ? Math.min(layoutViewportHeight, layoutViewportHeight - keyboardHeight)
      : Math.min(layoutViewportHeight, visualBottom);
    if (viewportHeight <= 0) return;
    const topSafe = 12;
    const bottomSafe = 24;
    let delta = 0;
    const visibleBottom = Math.max(topSafe, viewportHeight - bottomSafe);
    if (rect.bottom > visibleBottom) {
      delta = rect.bottom - visibleBottom;
    } else if (rect.top < topSafe) {
      delta = rect.top - topSafe;
    }
    if (Math.abs(delta) > 1) {
      applyScrollDelta(el, delta);
      return;
    }
    if (forceCenter && rect.bottom > layoutViewportHeight * 0.6) {
      el.scrollIntoView({ block: "center", inline: "nearest", behavior: "auto" });
    }
  };

  tryScroll();
  requestAnimationFrame(() => tryScroll());
  extraDelays.forEach((delay) => {
    setTimeout(() => tryScroll(), delay);
  });
}
