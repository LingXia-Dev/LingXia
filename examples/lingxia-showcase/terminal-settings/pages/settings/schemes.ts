import type { TerminalColorScheme, TerminalColorSchemeDetails } from '@lingxia/logic';

export type Tone = 'light' | 'dark';

/** A scheme as the page lists it: the host's, or one this app ships and imports on first use. */
export interface SchemeEntry {
  name: string;
  label: string;
  source: 'built_in' | 'imported' | 'packaged';
  /** Packaged schemes only. */
  author?: string;
  spdx?: string;
  upstream?: string;
  tone: Tone;
  scheme: TerminalColorScheme;
  /** The host has it; a packaged scheme it lacks is imported before it is chosen. */
  installed: boolean;
}

interface Packaged {
  name: string;
  displayName: string;
  author: string;
  upstream: string;
  spdx: string;
  scheme: TerminalColorScheme;
}

// Each one's license ships beside it in themes/<name>/LICENSE; check-themes keeps the two in step.
export const PACKAGED: readonly Packaged[] = [
  {
    name: 'dracula',
    displayName: 'Dracula',
    author: 'Dracula Theme',
    upstream: 'https://github.com/dracula/dracula-theme',
    spdx: 'MIT',
    scheme: {
      name: 'Dracula', background: '#282A36', foreground: '#F8F8F2',
      cursorColor: '#F8F8F2', selectionBackground: '#44475A',
      black: '#21222C', red: '#FF5555', green: '#50FA7B', yellow: '#F1FA8C',
      blue: '#BD93F9', purple: '#FF79C6', cyan: '#8BE9FD', white: '#F8F8F2',
      brightBlack: '#6272A4', brightRed: '#FF6E6E', brightGreen: '#69FF94',
      brightYellow: '#FFFFA5', brightBlue: '#D6ACFF', brightPurple: '#FF92DF',
      brightCyan: '#A4FFFF', brightWhite: '#FFFFFF',
    },
  },
  {
    name: 'nord',
    displayName: 'Nord',
    author: 'Sven Greb',
    upstream: 'https://github.com/nordtheme/nord',
    spdx: 'MIT',
    scheme: {
      name: 'Nord', background: '#2E3440', foreground: '#D8DEE9',
      cursorColor: '#D8DEE9', selectionBackground: '#434C5E',
      black: '#3B4252', red: '#BF616A', green: '#A3BE8C', yellow: '#EBCB8B',
      blue: '#81A1C1', purple: '#B48EAD', cyan: '#88C0D0', white: '#E5E9F0',
      brightBlack: '#4C566A', brightRed: '#BF616A', brightGreen: '#A3BE8C',
      brightYellow: '#EBCB8B', brightBlue: '#81A1C1', brightPurple: '#B48EAD',
      brightCyan: '#8FBCBB', brightWhite: '#ECEFF4',
    },
  },
  {
    name: 'solarized-dark',
    displayName: 'Solarized Dark',
    author: 'Ethan Schoonover',
    upstream: 'https://github.com/altercation/solarized',
    spdx: 'MIT',
    scheme: {
      name: 'Solarized Dark', background: '#002B36', foreground: '#839496',
      cursorColor: '#93A1A1', selectionBackground: '#073642',
      black: '#002B36', red: '#DC322F', green: '#859900', yellow: '#B58900',
      blue: '#268BD2', purple: '#D33682', cyan: '#2AA198', white: '#EEE8D5',
      brightBlack: '#073642', brightRed: '#CB4B16', brightGreen: '#586E75',
      brightYellow: '#657B83', brightBlue: '#839496', brightPurple: '#6C71C4',
      brightCyan: '#93A1A1', brightWhite: '#FDF6E3',
    },
  },
];

function channels(hex: string): [number, number, number] | null {
  let value = hex.trim().replace(/^#/, '');
  if (value.length === 3 || value.length === 4) value = [...value.slice(0, 3)].map((c) => c + c).join('');
  if (value.length === 8) value = value.slice(0, 6);
  if (!/^[0-9a-f]{6}$/i.test(value)) return null;
  return [0, 2, 4].map((at) => parseInt(value.slice(at, at + 2), 16) / 255) as [number, number, number];
}

/** Light or dark by the background's relative luminance; an unreadable one counts as dark. */
export function toneOf(scheme: TerminalColorScheme): Tone {
  const rgb = channels(scheme.background);
  if (!rgb) return 'dark';
  const [r, g, b] = rgb.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b >= 0.45 ? 'light' : 'dark';
}

function titleCase(name: string): string {
  return name.split('-').filter(Boolean).map((part) => part[0].toUpperCase() + part.slice(1)).join(' ');
}

/** The host's schemes over the packaged ones; a packaged scheme the host already has keeps its credits. */
export function mergeSchemes(host: readonly TerminalColorSchemeDetails[]): SchemeEntry[] {
  const byName = new Map<string, SchemeEntry>();
  for (const p of PACKAGED) {
    byName.set(p.name, {
      name: p.name, label: p.displayName, source: 'packaged', author: p.author, spdx: p.spdx,
      upstream: p.upstream, tone: toneOf(p.scheme), scheme: p.scheme, installed: false,
    });
  }
  for (const entry of host) {
    const packaged = byName.get(entry.name);
    byName.set(entry.name, packaged
      ? { ...packaged, scheme: entry.scheme, tone: toneOf(entry.scheme), installed: true }
      : {
        name: entry.name,
        label: entry.scheme.name || titleCase(entry.name),
        source: entry.source,
        tone: toneOf(entry.scheme),
        scheme: entry.scheme,
        installed: true,
      });
  }
  // Shipped defaults lead; a fresh import lands at the end, where the person looks for it.
  const rank = { built_in: 0, packaged: 1, imported: 2 } as const;
  return [...byName.values()].sort((a, b) => rank[a.source] - rank[b.source]);
}
