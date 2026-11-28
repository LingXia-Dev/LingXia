// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Bound, Reply, Counted, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, Invalid } from '../../core/src/TextArea/TextArea.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a text area when the answer may run to several lines: a note, a comment, a reply."}>
        <Bound />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A reply box that sends on Return"}>
          <Reply />
        </Stage>
        <Stage caption={"A character count against the limit"}>
          <Counted />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"State: Filled"}>
            <StateFilled />
          </Stage>
          <Stage caption={"State: Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Required, with an error"}>
          <Invalid />
        </Stage>
      </PageSection>
    </>
  );
}
