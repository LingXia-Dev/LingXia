// Generated from the doc file by scripts/generate.mjs; do not edit.
import { ViewOptions, Parts, Share, Beside, InSentence } from '../../core/src/Popover/Popover.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a popover for a few controls or lines that belong to the button that opens them."}>
        <ViewOptions />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The trigger, then the panel with its title"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"Above, lined up with the trigger's end"}>
          <Share />
        </Stage>
        <Stage caption={"At the side of the trigger; it flips or drops below when there is no room"}>
          <Beside />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"On a link in a sentence"}>
          <InSentence />
        </Stage>
      </PageSection>
    </>
  );
}
