import '../../theme/index.css';
import { Page } from '../../shared/ui';
import { Index } from '../../shared/view-lab/demo';
import { Group } from '../../shared/view-lab/groups';
import { SpecVideo } from '../../shared/view-lab/spec-video';
import { groups, type GroupId } from '../../shared/view-lab/text';
import type { View } from './$types';

// The index of the catalogue. Opened with `group`, it shows that group in place, under the tab
// bar; `spec=video` is the one fixture a spec needs under the tab bar.
export function render({ data }: View) {
  if (data.spec === 'video') return <Page testId="view-spec-video"><SpecVideo /></Page>;
  const chosen = groups.find(group => group.id === data.group)?.id as GroupId | undefined;
  return chosen ? <Group id={chosen} /> : <Index />;
}
