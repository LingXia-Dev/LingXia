import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { motionDemos } from '../../shared/view-lab/motion';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="motion" view={view} bodies={motionDemos} />;
}
