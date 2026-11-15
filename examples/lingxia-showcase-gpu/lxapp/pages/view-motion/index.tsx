import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { motionDemos } from '../../shared/view-lab/motion';

export function render() {
  return <GroupPage id="motion" bodies={motionDemos} />;
}
