import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { inputDemos } from '../../shared/view-lab/text-input';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="input" view={view} bodies={inputDemos} />;
}
