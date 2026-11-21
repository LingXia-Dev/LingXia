import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { accessibilityDemos } from '../../shared/view-lab/accessibility';

export function render() {
  return <GroupPage id="accessibility" bodies={accessibilityDemos} />;
}
