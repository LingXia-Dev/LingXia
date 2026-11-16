import '../../theme/index.css';
import { useView } from '@lingxia/view';
import { Page } from '../../shared/ui';
import { Index } from '../../shared/view-lab/demo';
import { Group } from '../../shared/view-lab/groups';
import { SpecVideo } from '../../shared/view-lab/spec-video';
import { groups, type GroupId } from '../../shared/view-lab/text';

// The index of the catalogue. Opened with `group`, it shows that group in place, under the tab
// bar; `spec=video` is the one fixture a spec needs under the tab bar.
export function render() {
  const { data, actions } = useView();
  const asked = data as { group?: string; spec?: string };
  if (asked.spec === 'video') return <Page testId="view-spec-video"><SpecVideo /></Page>;
  const chosen = groups.find(group => group.id === asked.group)?.id as GroupId | undefined;
  return chosen ? <Group id={chosen} /> : <Index onOpen={page => actions.open({ page })} />;
}
