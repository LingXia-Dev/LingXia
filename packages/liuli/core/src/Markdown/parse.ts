/** One run of a paragraph: text with its marks, and the link it belongs to. */
export interface MarkdownRun {
  text: string;
  isStrong?: boolean;
  isEmphasis?: boolean;
  isStruck?: boolean;
  isCode?: boolean;
  href?: string;
}

export type MarkdownAlign = 'start' | 'center' | 'end';

export type MarkdownBlock =
  | { type: 'heading'; level: number; runs: MarkdownRun[] }
  | { type: 'paragraph'; runs: MarkdownRun[] }
  | { type: 'list'; isOrdered: boolean; start: number; items: { isDone?: boolean; blocks: MarkdownBlock[] }[] }
  | { type: 'quote'; blocks: MarkdownBlock[] }
  | { type: 'code'; language: string; text: string }
  | { type: 'rule' }
  | { type: 'table'; align: MarkdownAlign[]; header: MarkdownRun[][]; rows: MarkdownRun[][][] };

type Marks = Omit<MarkdownRun, 'text'>;

const escapable = /^\\([\\`*_{}[\]()#+\-.!~|>])/;
const link = /^\[((?:[^[\]\\]|\\.)*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/;
const image = /^!\[((?:[^[\]\\]|\\.)*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/;
const autolink = /^<((?:https?:\/\/|mailto:)[^>\s]+)>/;
const wordy = /[\p{L}\p{N}]/u;

/** A paragraph's source as runs. A line end is a space; two spaces or a backslash before it is a line break. */
export function parseInline(source: string, marks: Marks = {}): MarkdownRun[] {
  const runs: MarkdownRun[] = [];
  let plain = '';
  const flush = () => { if (plain) runs.push({ text: plain, ...marks }); plain = ''; };
  const nest = (inner: string, extra: Marks) => { flush(); runs.push(...parseInline(inner, { ...marks, ...extra })); };
  for (let at = 0; at < source.length;) {
    const rest = source.slice(at);
    const escaped = escapable.exec(rest);
    if (escaped) { plain += escaped[1]; at += 2; continue; }
    if (rest[0] === '`') {
      const ticks = /^`+/.exec(rest)![0];
      const end = rest.indexOf(ticks, ticks.length);
      if (end > 0) {
        flush();
        runs.push({ text: rest.slice(ticks.length, end).replace(/\n/g, ' ').replace(/^ (.*) $/, '$1'), ...marks, isCode: true });
        at += end + ticks.length;
        continue;
      }
    }
    const pictured = image.exec(rest);
    if (pictured) { plain += pictured[1]; at += pictured[0].length; continue; }
    const linked = link.exec(rest);
    if (linked) { nest(linked[1], { href: linked[2] }); at += linked[0].length; continue; }
    const auto = autolink.exec(rest);
    if (auto) { flush(); runs.push({ text: auto[1].replace(/^mailto:/, ''), ...marks, href: auto[1] }); at += auto[0].length; continue; }
    const fence = /^(\*\*|__|~~|\*|_)/.exec(rest)?.[1];
    if (fence && /\S/.test(rest[fence.length] ?? '')) {
      // An underscore inside a word is part of the word.
      const inWord = fence[0] === '_' && wordy.test(source[at - 1] ?? '');
      let end = inWord ? -1 : rest.indexOf(fence, fence.length + 1);
      while (end > 0 && (/\s/.test(rest[end - 1]) || rest[end - 1] === '\\' || (fence[0] === '_' && wordy.test(rest[end + fence.length] ?? '')) || (fence.length === 1 && rest[end + 1] === fence))) {
        end = rest.indexOf(fence, end + (fence.length === 1 && rest[end + 1] === fence ? 2 : 1));
      }
      if (end > 0) {
        nest(rest.slice(fence.length, end), fence === '~~' ? { isStruck: true } : fence.length === 2 ? { isStrong: true } : { isEmphasis: true });
        at += end + fence.length;
        continue;
      }
    }
    const broken = /^( {2,}|\\)\n/.exec(rest);
    if (broken) { plain = plain.replace(/ +$/, '') + '\n'; at += broken[0].length; continue; }
    if (rest[0] === '\n') { plain = plain.replace(/ +$/, '') + ' '; at += 1; continue; }
    plain += rest[0];
    at += 1;
  }
  flush();
  return runs;
}

const heading = /^ {0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/;
const rule = /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/;
const fenced = /^ {0,3}(`{3,}|~{3,})\s*([^\s`]*)/;
const marker = /^(\s*)([-*+]|\d{1,9}[.)])(\s+)(.*)$/;
const divider = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

const cells = (line: string) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split(/(?<!\\)\|/).map(cell => cell.trim().replace(/\\\|/g, '|'));

/** Markdown as blocks: headings, paragraphs, lists with tasks, quotes, code, rules and tables. Anything else is text. */
export function parseMarkdown(source: string): MarkdownBlock[] {
  const lines = source.replace(/\r\n?/g, '\n').replace(/\t/g, '    ').split('\n');
  const blocks: MarkdownBlock[] = [];
  for (let at = 0; at < lines.length;) {
    const line = lines[at];
    if (line.trim() === '') { at += 1; continue; }
    const fence = fenced.exec(line);
    if (fence) {
      const body: string[] = [];
      for (at += 1; at < lines.length && !new RegExp(`^ {0,3}${fence[1][0]}{${fence[1].length},}\\s*$`).test(lines[at]); at += 1) body.push(lines[at]);
      blocks.push({ type: 'code', language: fence[2], text: body.join('\n') });
      at += 1;
      continue;
    }
    const title = heading.exec(line);
    if (title) { blocks.push({ type: 'heading', level: title[1].length, runs: parseInline(title[2]) }); at += 1; continue; }
    if (rule.test(line)) { blocks.push({ type: 'rule' }); at += 1; continue; }
    if (/^ {0,3}>/.test(line)) {
      const body: string[] = [];
      for (; at < lines.length && /^ {0,3}>/.test(lines[at]); at += 1) body.push(lines[at].replace(/^ {0,3}> ?/, ''));
      blocks.push({ type: 'quote', blocks: parseMarkdown(body.join('\n')) });
      continue;
    }
    const first = marker.exec(line);
    if (first) {
      const isOrdered = /\d/.test(first[2]);
      const items: { isDone?: boolean; blocks: MarkdownBlock[] }[] = [];
      while (at < lines.length) {
        const item = marker.exec(lines[at]);
        if (!item || item[1].length > first[1].length + 1 || /\d/.test(item[2]) !== isOrdered) break;
        const indent = item[1].length + item[2].length + item[3].length;
        const body = [item[4]];
        // Lines indented under the item, and blank lines between them, belong to it.
        for (at += 1; at < lines.length; at += 1) {
          const next = lines[at];
          if (next.trim() === '') { if (/^\s+\S/.test(lines[at + 1] ?? '') && (lines[at + 1].match(/^\s*/)![0].length >= indent)) body.push(''); else break; continue; }
          if (next.match(/^\s*/)![0].length < Math.min(indent, item[1].length + 2)) break;
          body.push(next.slice(Math.min(indent, next.match(/^\s*/)![0].length)));
        }
        const task = /^\[([ xX])\]\s+/.exec(body[0]);
        if (task) body[0] = body[0].slice(task[0].length);
        items.push({ ...(task ? { isDone: task[1] !== ' ' } : {}), blocks: parseMarkdown(body.join('\n')) });
        while (at < lines.length && lines[at].trim() === '' && marker.test(lines[at + 1] ?? '')) at += 1;
      }
      blocks.push({ type: 'list', isOrdered, start: isOrdered ? parseInt(first[2], 10) : 1, items });
      continue;
    }
    if (line.includes('|') && divider.test(lines[at + 1] ?? '') && lines[at + 1].includes('-')) {
      const align = cells(lines[at + 1]).map((cell): MarkdownAlign => (cell.endsWith(':') ? (cell.startsWith(':') ? 'center' : 'end') : 'start'));
      const header = cells(line).map(cell => parseInline(cell));
      const rows: MarkdownRun[][][] = [];
      for (at += 2; at < lines.length && lines[at].includes('|') && lines[at].trim() !== ''; at += 1) rows.push(cells(lines[at]).map(cell => parseInline(cell)));
      blocks.push({ type: 'table', align, header, rows });
      continue;
    }
    const body = [line];
    for (at += 1; at < lines.length && lines[at].trim() !== '' && !heading.test(lines[at]) && !fenced.test(lines[at]) && !rule.test(lines[at]) && !/^ {0,3}>/.test(lines[at]) && !marker.test(lines[at]); at += 1) body.push(lines[at]);
    blocks.push({ type: 'paragraph', runs: parseInline(body.map(each => each.replace(/^\s+/, '')).join('\n')) });
  }
  return blocks;
}

/** The runs' text alone, as assistive tech names a cell or an item. */
export const plainText = (runs: readonly MarkdownRun[]) => runs.map(run => run.text).join('');
