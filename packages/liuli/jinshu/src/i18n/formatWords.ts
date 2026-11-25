import type { Problem } from '../schema/types';

type Words = (key: `@jinshu.${string}`, args?: Readonly<Record<string, unknown>>) => string;

const blockName = (type: string, word: Words) => word('@jinshu.blockType', { type: type.startsWith('x-') ? 'custom' : type, level: '' }).trim();

/**
 * The `FORMAT` problems as words a person reads, one per distinct thing, in order. The document's `id`
 * is left out: the app's own key or file name is the document's identity.
 */
export function formatWordsWith(problems: readonly Problem[], word: Words): string[] {
  const out: string[] = [];
  for (const p of problems) {
    const s = p.subject;
    if (p.code !== 'FORMAT' || !s) continue;
    let text: string;
    if ('rule' in s) text = word('@jinshu.format.rule', { rule: s.rule });
    else if ('mark' in s) text = word('@jinshu.format.mark', { mark: s.mark });
    else if ('field' in s) {
      if (s.field === 'id' && s.block === undefined) continue;
      const field = word('@jinshu.format.field', { field: s.field });
      text = s.block === undefined ? field : word('@jinshu.format.blockField', { block: blockName(s.block, word), field });
    } else text = blockName(s.block, word);
    if (!out.includes(text)) out.push(text);
  }
  return out;
}

/** `formatWordsWith` joined as one line. */
export function formatLineWith(problems: readonly Problem[], word: Words): string {
  return formatWordsWith(problems, word).join(word('@jinshu.format.separator'));
}
