// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Workspace } from '../../../core/src/SplitView/SplitView.examples';
import { GalleryPage, Lead, PageSection, Stage } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a split view for a workspace with a sidebar on a wide window."} bare>
        <Workspace />
      </Lead>
    </GalleryPage>
  );
}
