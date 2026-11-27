import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { webDemos } from '../../shared/view-lab/elements';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="web" view={view} bodies={webDemos} />;
}
