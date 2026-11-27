import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { controlDemos } from '../../shared/view-lab/elements';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="controls" view={view} bodies={controlDemos} />;
}
