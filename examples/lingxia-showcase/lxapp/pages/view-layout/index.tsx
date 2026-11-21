import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { layoutDemos } from '../../shared/view-lab/layout';

export function render() {
  return <GroupPage id="layout" bodies={layoutDemos} />;
}
