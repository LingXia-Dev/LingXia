import { reportHostOverlays } from 'liblingxia.so';

/** One element of a host overlay as automation reads it; bounds are window vp. */
class HostOverlayElement {
  source: string = '';
  key: string = '';
  label: () => string = () => '';
  role: string = 'text';
  bounds: number[] = [0, 0, 0, 0];
  press: (() => void) | null = null;
}

/** The wire shape Rust reads. */
class ReportedElement {
  key: string = '';
  label: string = '';
  role: string = 'text';
  bounds: number[] = [0, 0, 0, 0];
}

/**
 * The host's own overlays (toast, modal, action sheet) as window chrome for automation: their
 * labels, bounds, and the press a window tap at those bounds runs. HarmonyOS keeps window input
 * injection from apps, so these are the only window elements a tap reaches.
 */
export class HostOverlays {
  private static elements: Map<string, HostOverlayElement> = new Map();
  /** Overlays on screen; a closing one keeps its nodes for a frame but takes no more taps. */
  private static shown: Set<string> = new Set();

  /**
   * Place element `id` of overlay `source` where `area` says; `press` runs on a tap. Labels are
   * read when reported: a dialog shown again in the same frame keeps its nodes, so no new area
   * arrives for it.
   */
  static place(source: string, id: string, label: () => string, role: string, area: Area,
    press: (() => void) | null = null): void {
    const element = new HostOverlayElement();
    element.source = source;
    element.key = `${source}:${id}`;
    element.label = label;
    element.role = role;
    element.bounds = [
      Number(area.globalPosition.x ?? 0), Number(area.globalPosition.y ?? 0),
      Number(area.width), Number(area.height),
    ];
    element.press = press;
    HostOverlays.elements.set(element.key, element);
    HostOverlays.report();
  }

  /** Element `id` of overlay `source` left the screen. */
  static forget(source: string, id: string): void {
    if (HostOverlays.elements.delete(`${source}:${id}`)) {
      HostOverlays.report();
    }
  }

  /** Overlay `source` shows, possibly new content on the nodes it already has. */
  static show(source: string): void {
    HostOverlays.shown.add(source);
    HostOverlays.report();
  }

  /** Overlay `source` closes: its elements stop being reported before its nodes go. */
  static hide(source: string): void {
    if (HostOverlays.shown.delete(source)) {
      HostOverlays.report();
    }
  }

  /** A window tap landed on element `key`; false when it is gone or presses nothing. */
  static press(key: string): boolean {
    const element = HostOverlays.elements.get(key);
    const press = element !== undefined && HostOverlays.shown.has(element.source) ? element.press : null;
    if (press === null) {
      return false;
    }
    press();
    return true;
  }

  private static report(): void {
    const reported: ReportedElement[] = Array.from(HostOverlays.elements.values())
      .filter((element: HostOverlayElement) => HostOverlays.shown.has(element.source))
      .map(
      (element: HostOverlayElement) => {
        const out = new ReportedElement();
        out.key = element.key;
        out.label = element.label();
        out.role = element.role;
        out.bounds = element.bounds;
        return out;
      });
    reportHostOverlays(JSON.stringify(reported));
  }
}
