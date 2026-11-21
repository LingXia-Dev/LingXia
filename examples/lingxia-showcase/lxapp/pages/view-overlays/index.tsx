import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { overlayDemos } from '../../shared/view-lab/overlays';

export function render() {
  return <GroupPage id="overlays" bodies={overlayDemos} />;
}
