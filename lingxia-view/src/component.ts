export type Rect = { x: number; y: number; width: number; height: number };

let autoId = 0;

export function ensureComponentId(el: HTMLElement, prefix: string, currentId?: string | null): string {
  const attrId = el.getAttribute("id");
  if (attrId && attrId.length > 0) {
    return attrId;
  }
  if (currentId && currentId.length > 0) {
    return currentId;
  }
  autoId += 1;
  const id = `${prefix}-${Date.now().toString(36)}-${autoId.toString(36)}`;
  el.setAttribute("id", id);
  return id;
}

export function rectEquals(a: Rect | null, b: Rect, epsilon = 0): boolean {
  if (!a) return false;
  return (
    Math.abs(a.x - b.x) <= epsilon &&
    Math.abs(a.y - b.y) <= epsilon &&
    Math.abs(a.width - b.width) <= epsilon &&
    Math.abs(a.height - b.height) <= epsilon
  );
}

export class SameLevelUpdateState {
  private lastPropsJson: string | null = null;
  private lastRect: Rect | null = null;
  private lastZIndex: number | null = null;

  reset() {
    this.lastPropsJson = null;
    this.lastRect = null;
    this.lastZIndex = null;
  }

  shouldSend(rect: Rect, props: Record<string, unknown> | null, zIndex: number, force = false): boolean {
    const propsJson = props === null ? this.lastPropsJson : JSON.stringify(props);
    const rectChanged = !rectEquals(this.lastRect, rect);
    const propsChanged = props === null ? false : propsJson !== this.lastPropsJson;
    const zChanged = this.lastZIndex !== zIndex;

    const changed = force || rectChanged || propsChanged || zChanged;
    if (!changed) return false;

    if (props !== null) {
      this.lastPropsJson = propsJson;
    }
    this.lastRect = rect;
    this.lastZIndex = zIndex;
    return true;
  }
}
