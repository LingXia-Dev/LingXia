// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Stay, Empty, StateInvalid, StateDisabled } from '../../core/src/DateRangeInput/DateRangeInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a date range input when a form needs a first and a last day."}>
        <Stay />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Nothing chosen yet, with a placeholder"}>
          <Empty />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid stack>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
