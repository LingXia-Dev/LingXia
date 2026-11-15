import { demos, groups, type GroupId } from '../shared/view-lab/text';

type DemoName = { [G in GroupId]: keyof (typeof demos)[G] }[GroupId];

/** The catalogue page a demo lives on. */
export function pageOf(demo: DemoName): string {
  const group = groups.find(group => demo in demos[group.id]);
  if (!group) throw new Error(`no catalogue group holds the demo ${String(demo)}`);
  return group.page;
}

/** A spec's `start` for a demo: its page, in English unless the spec is about another language. */
export function openDemo(demo: DemoName, lang: 'en' | 'zh' = 'en'): { page: string; query: { lang: string } } {
  return { page: pageOf(demo), query: { lang } };
}
