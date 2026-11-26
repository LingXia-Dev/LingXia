// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Checkout, Across, NeedsAttention } from '../../core/src/Stepper/Stepper.examples';
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
        <Stage caption={"A step to fix, and a finished step that cannot be gone back to"}>
          <NeedsAttention />
        </Stage>
      </PageSection>
    </>
  );
}
