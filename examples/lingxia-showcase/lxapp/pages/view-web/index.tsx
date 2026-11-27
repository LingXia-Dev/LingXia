import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { webDemos } from '../../shared/view-lab/elements';

export function render() {
  return <GroupPage id="web" bodies={webDemos} />;
}
