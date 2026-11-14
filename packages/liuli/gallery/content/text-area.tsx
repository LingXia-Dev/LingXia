// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Bound, Reply, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, Invalid } from '../../core/src/TextArea/TextArea.examples';
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
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Filled"}>
            <StateFilled />
          </Stage>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"Disabled"}>
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
