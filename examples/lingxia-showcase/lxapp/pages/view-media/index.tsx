import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { mediaDemos } from '../../shared/view-lab/elements';

export function render() {
  return <GroupPage id="media" bodies={mediaDemos} />;
}
