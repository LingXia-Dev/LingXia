import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { appearanceDemos } from '../../shared/view-lab/appearance';

export function render() {
  return <GroupPage id="appearance" bodies={appearanceDemos} />;
}
