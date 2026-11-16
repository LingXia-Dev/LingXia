// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Phones, Links, StateDisabled } from '../../core/src/ListInput/ListInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a list input when one field holds several values that are typed, not picked."}>
        <Phones />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Empty and required: the error says what to add"}>
          <Links />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid stack>
          <Stage caption={"Disabled, with the reason"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
