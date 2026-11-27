import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { appearanceDemos } from '../../shared/view-lab/appearance';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="appearance" view={view} bodies={appearanceDemos} />;
}
