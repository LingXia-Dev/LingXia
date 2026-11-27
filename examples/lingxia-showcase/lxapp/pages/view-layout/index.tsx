import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { layoutDemos, pinnedRoom } from '../../shared/view-lab/layout';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="layout" view={view} bodies={layoutDemos} endRoom={pinnedRoom} />;
}
