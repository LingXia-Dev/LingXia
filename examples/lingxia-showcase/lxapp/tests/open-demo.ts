import { demos, groups, type GroupId } from '../shared/view-lab/text';

type DemoName = { [G in GroupId]: keyof (typeof demos)[G] }[GroupId];

const groupOf = (demo: DemoName) => {
  const group = groups.find(group => demo in demos[group.id]);
  if (!group) throw new Error(`no catalogue group holds the demo ${String(demo)}`);
  return group;
};

/** The catalogue page a demo lives on. */
export function pageOf(demo: DemoName): string {
  return groupOf(demo).page;
}

/** The test id of the page a demo is on, wherever that group is shown. */
export function rootOf(demo: DemoName): string {
  return `view-group-${groupOf(demo).id}`;
}

/** A spec's `start` for a demo: its page, in English unless the spec is about another language. */
export function openDemo(demo: DemoName, lang: 'en' | 'zh' = 'en'): { page: string; params: { lang: string } } {
  return { page: pageOf(demo), params: { lang } };
}

/** A spec's `start` for a demo under the tab bar: the View tab showing the demo's group in place. */
export function openUnderTabs(demo: DemoName, lang: 'en' | 'zh' = 'en'): { page: string; params: { group: string; lang: string } } {
  return { page: 'view', params: { group: groupOf(demo).id, lang } };
}
