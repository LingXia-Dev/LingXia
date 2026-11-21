export type LineKind = 'context' | 'added' | 'removed' | 'header';

/** Each line's part in a unified diff. */
export function diffKinds(lines: readonly string[]): LineKind[] {
  return lines.map((line, index) => {
    if (line.startsWith('@@')) return 'header';
    const next = lines[index + 1] ?? '', after = lines[index + 2] ?? '';
    if (line.startsWith('--- ') && next.startsWith('+++ ') && after.startsWith('@@')) return 'header';
    if (line.startsWith('+++ ') && (lines[index - 1] ?? '').startsWith('--- ') && next.startsWith('@@')) return 'header';
    if (line.startsWith('+')) return 'added';
    if (line.startsWith('-')) return 'removed';
    return 'context';
  });
}
