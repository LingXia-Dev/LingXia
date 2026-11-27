import { expect, test } from '@lingxia/test';
import type { PageTreeNode } from '@lingxia/types/automation';
import manifest from '../lxapp.json';

// Every gallery page at 130 % text, and at 200 % text in high contrast: no words pushed off the
// screen sideways, and no words drawn over other words. Read from the page's accessibility tree,
// whose bounds are where each element is laid out.
test.configure({ tags: ['text-size'] });

const looks = [
  { textScale: 1.3, contrast: 'standard' as const, label: '130%' },
  { textScale: 2, contrast: 'high' as const, label: '200% high contrast' },
];

/** Problems the gallery shows today, each `page (look): what`; fix one and take it off. */
const known = new Set<string>([]);

const pages = manifest.pages.map(page => page.name).filter(name => name !== 'home' && !name.startsWith('category-'));

type Leaf = { name: string; words: boolean; box: [number, number, number, number]; scrolled: boolean };

/** Named leaves, each marked when it sits inside a scrolling region, which may run past the screen. */
function leaves(nodes: PageTreeNode[], scrolled = false): Leaf[] {
  return nodes.flatMap(node => {
    const inside = scrolled || node.scroll !== undefined;
    if (node.children?.length) return leaves(node.children, inside);
    if (!node.name.trim() || node.offscreen) return [];
    return [{ name: node.name, words: node.role === 'text' || node.role === 'heading', box: node.bounds, scrolled: inside }];
  });
}

const inside = (a: string, b: string) => a.includes(b) || b.includes(a);

const overlap = (a: Leaf['box'], b: Leaf['box']) =>
  Math.min(a[0] + a[2], b[0] + b[2]) - Math.max(a[0], b[0]) > 2 && Math.min(a[1] + a[3], b[1] + b[3]) - Math.max(a[1], b[1]) > 2;

for (const page of pages) {
  test(`text size: ${page}`, { start: { page }, timeout: 90_000 }, async (t) => {
    const view = t.app.view;
    const problems: string[] = [];
    for (const look of looks) {
      await view.environment({ textScale: look.textScale, contrast: look.contrast, still: true });
      await view.settled().catch(() => {});
      const { width } = await view.viewport();
      const found = leaves((await t.app.logic.eval(({ lx }) => lx.automation().lxapp().page.tree())).nodes);
      const before = problems.length;
      const say = (what: string) => {
        const line = `${page} (${look.label}): ${what}`;
        if (!known.has(line)) problems.push(line);
      };
      for (const { name, box: [left, , w], scrolled } of found)
        if (!scrolled && (left < -1 || left + w > width + 1)) say(`"${name}" runs off the screen`);
      for (let i = 0; i < found.length; i += 1)
        for (let j = i + 1; j < found.length; j += 1)
          // Controls laid over content and inline text inside its sentence overlap on purpose.
          if (found[i].words && found[j].words && !inside(found[i].name, found[j].name) && overlap(found[i].box, found[j].box))
            say(`"${found[i].name}" overlaps "${found[j].name}"`);
      if (problems.length > before) await t.attach(`${page}-${look.textScale * 100}.png`, await view.screenshot());
    }
    expect(problems, problems.join('\n') || 'no problems').toEqual([]);
  });
}
