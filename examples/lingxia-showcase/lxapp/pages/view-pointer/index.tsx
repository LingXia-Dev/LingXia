import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { pointerDemos } from '../../shared/view-lab/gestures';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="pointer" view={view} bodies={pointerDemos} />;
}
