import '../../theme/index.css';
import { Page } from '../../shared/ui';
import { SpecVideo } from '../../shared/view-lab/spec-video';
import type { View } from './$types';

// For specs only: not reachable from the app.
export function render({ data }: View) {
  return <Page testId="spec-video"><SpecVideo packaged={data.packagedMedia} /></Page>;
}
