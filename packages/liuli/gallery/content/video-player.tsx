// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Standard, UnderThePicture } from '../../core/src/VideoPlayer/VideoPlayer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use VideoPlayer to show a video with standard controls."} bare>
        <Standard />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Controls under the picture"} bare>
          <UnderThePicture />
        </Stage>
      </PageSection>
    </>
  );
}
