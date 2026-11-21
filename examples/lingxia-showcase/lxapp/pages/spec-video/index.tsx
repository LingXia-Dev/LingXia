import '../../theme/index.css';
import { Page } from '../../shared/ui';
import { SpecVideo } from '../../shared/view-lab/spec-video';

// For specs only: not reachable from the app.
export function render() {
  return <Page testId="spec-video"><SpecVideo /></Page>;
}
