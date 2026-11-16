import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { drawingDemos } from '../../shared/view-lab/elements';

export function render() {
  return <GroupPage id="drawing" bodies={drawingDemos} />;
}
