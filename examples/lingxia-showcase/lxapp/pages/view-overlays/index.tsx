import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { overlayDemos } from '../../shared/view-lab/overlays';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="overlays" view={view} bodies={overlayDemos} />;
}
