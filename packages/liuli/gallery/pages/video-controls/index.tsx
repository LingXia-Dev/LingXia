// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Overlay, Bar } from '../../../core/src/VideoControls/VideoControls.examples';
import { GalleryPage, Lead, PageSection, Stage } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use VideoControls beside your own <video> when VideoPlayer does not fit the layout."} bare>
        <Overlay />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A row under the picture"} bare>
          <Bar />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
