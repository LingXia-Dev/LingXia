// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Country, Sizes, StateEmpty, StateChosen, StateInvalid, StateDisabled, Required } from '../../core/src/Select/Select.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a select when there are too many options to show at once, or the choice matters less than the space."}>
        <Country />
      </Lead>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Nothing chosen"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"State: Chosen"}>
            <StateChosen />
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
        <Stage caption={"Inviting someone, after a send with no role"}>
          <Required />
        </Stage>
      </PageSection>
    </>
  );
}
