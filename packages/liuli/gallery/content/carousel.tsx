// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Photos, Welcome, Looping, Rail } from '../../core/src/Carousel/Carousel.examples';
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
        <Stage caption={"Buttons over the edges; past the last photo comes the first"}>
          <Looping />
        </Stage>
        <Stage caption={"A rail of 160-point cards that stops on each"}>
          <Rail />
        </Stage>
      </PageSection>
    </>
  );
}
