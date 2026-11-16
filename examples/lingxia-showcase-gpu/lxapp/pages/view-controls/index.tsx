import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { controlDemos } from '../../shared/view-lab/elements';

export function render() {
  return <GroupPage id="controls" bodies={controlDemos} />;
}
