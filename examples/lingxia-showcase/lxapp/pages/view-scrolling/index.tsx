import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { scrollingDemos } from '../../shared/view-lab/scrolling';

export function render() {
  return <GroupPage id="scrolling" bodies={scrollingDemos} />;
}
