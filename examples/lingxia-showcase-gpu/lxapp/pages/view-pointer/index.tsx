import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { pointerDemos } from '../../shared/view-lab/gestures';

export function render() {
  return <GroupPage id="pointer" bodies={pointerDemos} />;
}
