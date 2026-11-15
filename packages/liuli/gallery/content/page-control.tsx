// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Welcome, OnMedia } from '../../core/src/PageControl/PageControl.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a page control to show which of a few pages is in view."}>
        <Welcome />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Over a picture, in the on-media tone"}>
          <OnMedia />
        </Stage>
      </PageSection>
    </>
  );
}
