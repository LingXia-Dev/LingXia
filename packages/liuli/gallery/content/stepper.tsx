// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Checkout, Across } from '../../core/src/Stepper/Stepper.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a stepper for a task with a known number of steps done in order."}>
        <Checkout />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Horizontal, for up to four short labels"}>
          <Across />
        </Stage>
      </PageSection>
    </>
  );
}
