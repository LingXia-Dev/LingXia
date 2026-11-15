// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Photos, Welcome } from '../../core/src/Carousel/Carousel.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a carousel for a few like things to swipe through."}>
        <Photos />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Content pages, with the page shared with a button"}>
          <Welcome />
        </Stage>
      </PageSection>
    </>
  );
}
