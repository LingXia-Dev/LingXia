/**
 * Last laid-out capsule and page origin, in window vp.
 * View `capsuleRect` is this pill minus the page origin (WebView CSS space).
 */

let capsuleX = 0;
let capsuleY = 0;
let capsuleW = 0;
let capsuleH = 0;
let capsuleReady = false;
let pageX = 0;
let pageY = 0;
let pageReady = false;
let listener: (() => void) | null = null;

function roundVp(value: number): number {
  return Math.round(value * 2) / 2;
}

export function setCapsuleGeometryListener(next: (() => void) | null): void {
  listener = next;
}

export function noteCapsuleFrame(x: number, y: number, width: number, height: number): void {
  const nx = roundVp(x);
  const ny = roundVp(y);
  const nw = roundVp(width);
  const nh = roundVp(height);
  const ready = nw > 0 && nh > 0;
  const changed =
    !capsuleReady || nx !== capsuleX || ny !== capsuleY || nw !== capsuleW || nh !== capsuleH;
  capsuleX = nx;
  capsuleY = ny;
  capsuleW = nw;
  capsuleH = nh;
  capsuleReady = ready;
  if (changed && ready) {
    listener?.();
  }
}

export function notePageOrigin(x: number, y: number): void {
  const nx = roundVp(x);
  const ny = roundVp(y);
  const changed = !pageReady || nx !== pageX || ny !== pageY;
  pageX = nx;
  pageY = ny;
  pageReady = true;
  if (changed && capsuleReady) {
    listener?.();
  }
}

export function clearCapsuleGeometry(): void {
  capsuleReady = false;
  pageReady = false;
}

export function measuredCapsuleRectJson(): string {
  if (!capsuleReady) {
    return 'null';
  }
  const left = capsuleX - (pageReady ? pageX : 0);
  const top = capsuleY - (pageReady ? pageY : 0);
  return JSON.stringify({
    width: capsuleW,
    height: capsuleH,
    top: top,
    left: left,
    right: left + capsuleW,
    bottom: top + capsuleH
  });
}
