import '../../theme/index.css';
import { useView } from '@lingxia/view';
import { Index } from '../../shared/view-lab/demo';

export function render() {
  const { actions } = useView();
  return <Index onOpen={page => actions.open({ page })} />;
}
