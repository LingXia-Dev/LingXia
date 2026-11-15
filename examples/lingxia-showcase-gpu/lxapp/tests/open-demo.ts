import { demos, groups, type GroupId } from '../shared/view-lab/text';

type DemoName = { [G in GroupId]: keyof (typeof demos)[G] }[GroupId];

/** The catalogue page a demo lives on, for a spec's `start: { page }`. */
export function pageOf(demo: DemoName): string {
  const group = groups.find(group => demo in demos[group.id]);
  if (!group) throw new Error(`no catalogue group holds the demo ${String(demo)}`);
  return group.page;
}
