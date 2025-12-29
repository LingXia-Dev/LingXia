import type { SameLevelMessage } from '../types';

type SameLevelCoverageState = {
  installed: boolean;
  coveredById: Map<string, boolean>;
  coveredCount: number;
  anyCovered: boolean;
  scheduled: boolean;
  hasAnyVideo: boolean;
  scrollLocked: boolean;
  scrollY: number;
  htmlOverflow: string;
  bodyOverflow: string;
  bodyPosition: string;
  bodyTop: string;
  bodyWidth: string;
};

type InstallArgs = {
  os: string;
  send: (message: SameLevelMessage) => void;
};

export function installSameLevelCoverageMonitor({ os, send }: InstallArgs): void {
  if (typeof window === 'undefined') return;
  if (typeof document === 'undefined') return;
  const isIOS = os === 'iOS';
  const isAndroid = os === 'Android';
  if (!isIOS && !isAndroid) return;

  const key = Symbol.for('LingXia.SameLevelCoverageMonitor');
  const existing = (window as any)[key] as SameLevelCoverageState | undefined;
  if (existing?.installed) return;

  const state: SameLevelCoverageState = {
    installed: true,
    coveredById: new Map(),
    coveredCount: 0,
    anyCovered: false,
    scheduled: false,
    hasAnyVideo: false,
    scrollLocked: false,
    scrollY: 0,
    htmlOverflow: '',
    bodyOverflow: '',
    bodyPosition: '',
    bodyTop: '',
    bodyWidth: '',
  };
  (window as any)[key] = state;

  function lockScroll(): void {
    if (!isIOS) return;
    if (state.scrollLocked) return;
    const html = document.documentElement;
    const body = document.body;
    if (!html || !body) return;

    state.scrollLocked = true;
    state.scrollY = window.scrollY || html.scrollTop || 0;

    state.htmlOverflow = html.style.overflow || '';
    state.bodyOverflow = body.style.overflow || '';
    state.bodyPosition = body.style.position || '';
    state.bodyTop = body.style.top || '';
    state.bodyWidth = body.style.width || '';

    html.style.overflow = 'hidden';
    body.style.overflow = 'hidden';
    body.style.position = 'fixed';
    body.style.top = `${-state.scrollY}px`;
    body.style.width = '100%';
  }

  function unlockScroll(): void {
    if (!isIOS) return;
    if (!state.scrollLocked) return;
    const html = document.documentElement;
    const body = document.body;
    if (!html || !body) return;

    state.scrollLocked = false;
    html.style.overflow = state.htmlOverflow;
    body.style.overflow = state.bodyOverflow;
    body.style.position = state.bodyPosition;
    body.style.top = state.bodyTop;
    body.style.width = state.bodyWidth;

    window.scrollTo(0, state.scrollY);
  }

  function isElementCovered(el: Element): boolean {
    const rect = (el as HTMLElement).getBoundingClientRect?.();
    if (!rect) return false;
    if (rect.width < 2 || rect.height < 2) return false;

    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    if (x < 0 || y < 0 || x >= window.innerWidth || y >= window.innerHeight)
      return false;

    const top = document.elementFromPoint?.(x, y);
    if (!top) return false;
    if (top === document.documentElement || top === document.body) return false;
    return !(top === el || el.contains(top));
  }

  function check(): void {
    state.scheduled = false;

    const nodes = document.querySelectorAll<HTMLElement>('lx-video[id]');
    state.hasAnyVideo = nodes.length > 0;
    if (!state.hasAnyVideo && state.coveredById.size === 0 && !state.anyCovered) {
      return;
    }
    const seenIds = new Set<string>();

    for (const node of Array.from(nodes)) {
      const id = node.id;
      if (!id) continue;
      seenIds.add(id);
      const covered = isElementCovered(node);
      const prev = state.coveredById.get(id);
      if (prev === covered) continue;
      if (prev === true) state.coveredCount -= 1;
      if (covered === true) state.coveredCount += 1;
      state.coveredById.set(id, covered);
      send({ id, action: 'component.coverage', covered });
    }

    const staleIds: string[] = [];
    for (const [id, covered] of state.coveredById.entries()) {
      if (seenIds.has(id)) continue;
      staleIds.push(id);
      if (covered === true) state.coveredCount -= 1;
    }
    for (const id of staleIds) {
      state.coveredById.delete(id);
      send({ id, action: 'component.coverage', covered: false });
    }

    const anyCovered = state.coveredCount > 0;
    if (anyCovered !== state.anyCovered) {
      state.anyCovered = anyCovered;
      if (isIOS) {
        if (anyCovered) lockScroll();
        else unlockScroll();
      }
    }
  }

  function schedule(): void {
    if (state.scheduled) return;
    state.scheduled = true;
    requestAnimationFrame(check);
  }

  function onScroll(): void {
    if (!state.hasAnyVideo && state.coveredById.size === 0) return;
    schedule();
  }

  const observer = new MutationObserver(schedule);
  observer.observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
  });
  window.addEventListener('scroll', onScroll, { capture: true, passive: true });
  window.addEventListener('resize', schedule);
  schedule();
}
