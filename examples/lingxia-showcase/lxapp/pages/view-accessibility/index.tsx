import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { accessibilityDemos } from '../../shared/view-lab/accessibility';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="accessibility" view={view} bodies={accessibilityDemos} />;
}
