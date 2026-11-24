import '../../theme/index.css';
import { GroupPage } from '../../shared/view-lab/demo';
import { layoutDemos, pinnedRoom } from '../../shared/view-lab/layout';

export function render() {
  return <GroupPage id="layout" bodies={layoutDemos} endRoom={pinnedRoom} />;
}
