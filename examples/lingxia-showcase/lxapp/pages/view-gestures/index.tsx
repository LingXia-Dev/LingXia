import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { gestureDemos } from '../../shared/view-lab/gestures';

export function render() {
  return <GroupPage id="gestures" bodies={gestureDemos} />;
}
