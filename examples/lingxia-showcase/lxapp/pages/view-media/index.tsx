import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { mediaDemos } from '../../shared/view-lab/elements';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="media" view={view} bodies={mediaDemos} />;
}
