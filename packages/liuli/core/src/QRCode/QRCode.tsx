import { cx } from '@lingxia/view';
import { qrCodeRecipe as r } from './QRCode.recipe';
import { dataOf } from '../internal/data';
import { warnOnce } from '../internal/warn';
import { encodeQR, qrRoom, type QRLevel } from './encode';

export type QRCodeSize = 'sm' | 'md' | 'lg';
export type { QRLevel };

export interface QRCodeProps {
  /** What the code holds: a link, a join code, Wi-Fi details. Text of any language. */
  value: string;
  /** What the code is for, read in its place: "Code to join Harbour Café Guest". */
  label: string;
  /** 96, 160 or 240 points square. @default 'md' */
  size?: QRCodeSize;
  /** How much of the code may be lost and still read: L 7 %, M 15 %, Q 25 %, H 30 %. Higher levels make denser codes. @default 'M' */
  level?: QRLevel;
  className?: string;
  'data-testid'?: string;
}

// The largest version whose paths stay well inside the View's vector budget.
const maxVersion = 15;
const quiet = 4;
const cache = new Map<string, { viewBox: [number, number, number, number]; paths: { d: string; fill: string }[] }>();

/** The code's modules as filled rectangles: runs along each row, merged down while the rows below repeat them. */
function source(value: string, level: QRLevel) {
  const key = `${level}\u0000${value}`;
  const known = cache.get(key);
  if (known) return known;
  const modules = encodeQR(value, level, maxVersion);
  const size = modules.length;
  const used = modules.map(row => row.map(() => false));
  const pieces: string[] = [];
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    if (!modules[y][x] || used[y][x]) continue;
    let width = 0;
    while (x + width < size && modules[y][x + width] && !used[y][x + width]) width++;
    const sameRun = (row: number) =>
      (x === 0 || !modules[row][x - 1]) && (x + width === size || !modules[row][x + width]) &&
      modules[row].slice(x, x + width).every((dark, k) => dark && !used[row][x + k]);
    let height = 1;
    while (y + height < size && sameRun(y + height)) height++;
    for (let dy = 0; dy < height; dy++) for (let k = 0; k < width; k++) used[y + dy][x + k] = true;
    pieces.push(`M${x + quiet} ${y + quiet}h${width}v${height}h-${width}z`);
  }
  // A path holds at most 1,024 commands; five per rectangle.
  const paths = [{ d: `M0 0h${size + 2 * quiet}v${size + 2 * quiet}H0z`, fill: '#ffffff' }];
  for (let i = 0; i < pieces.length; i += 200) paths.push({ d: pieces.slice(i, i + 200).join(''), fill: '#000000' });
  const result = { viewBox: [0, 0, size + 2 * quiet, size + 2 * quiet] as [number, number, number, number], paths };
  if (cache.size > 32) cache.delete(cache.keys().next().value as string);
  cache.set(key, result);
  return result;
}

/** Whether `value` fits a QRCode at `level`; branch on it before rendering a value that comes from data. */
export function fitsQRCode(value: string, level: QRLevel = 'M'): boolean {
  const { bytes, limit } = qrRoom(value, level, maxVersion);
  return bytes <= limit;
}

/** A QR code for a phone camera to read: a link to open, a code to join with. */
export function QRCode({ value, label, size = 'md', level = 'M', className, 'data-testid': testId, ...attributes }: QRCodeProps) {
  const { bytes, limit } = qrRoom(value, level, maxVersion);
  const fits = bytes <= limit;
  if (!fits) warnOnce(`QRCode "${label}": the value is ${bytes} bytes; at level ${level} a code holds at most ${limit}. It is drawn empty; check with fitsQRCode().`);
  const code = fits ? source(value, level) : null;
  return (
    <view role="img" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.size[size], !code && r.root.empty, className)}>
      {code && <vector aria-hidden paths={code.paths} viewBox={code.viewBox} className={r.code.base} />}
    </view>
  );
}
