import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { textDemos } from '../../shared/view-lab/text-input';

export function render() {
  return <GroupPage id="text" bodies={textDemos} />;
}
