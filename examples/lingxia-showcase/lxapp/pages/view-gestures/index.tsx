import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { gestureDemos } from '../../shared/view-lab/gestures';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="gestures" view={view} bodies={gestureDemos} />;
}
