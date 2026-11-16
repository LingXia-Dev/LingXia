import '../../theme/index.css';
import { useView } from '@lingxia/view';
import { Index } from '../../shared/view-lab/demo';
import { Group } from '../../shared/view-lab/groups';
import { groups, type GroupId } from '../../shared/view-lab/text';

// The index of the catalogue. Opened with `group`, it shows that group in place, under the tab bar.
export function render() {
  const { data, actions } = useView();
  const asked = data as { group?: string };
  const chosen = groups.find(group => group.id === asked.group)?.id as GroupId | undefined;
  return chosen ? <Group id={chosen} /> : <Index onOpen={page => actions.open({ page })} />;
}
