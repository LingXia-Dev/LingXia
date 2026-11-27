import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { scrollingDemos } from '../../shared/view-lab/scrolling';
import type { View } from './$types';

export function render(view: View) {
  return <GroupPage id="scrolling" view={view} bodies={scrollingDemos} />;
}
