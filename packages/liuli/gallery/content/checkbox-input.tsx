// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, Sizes, StateOff, StateOn, StateInvalid, StateDisabled, StateMixed, Controlled, Saved } from '../../core/src/CheckboxInput/CheckboxInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a checkbox for one on/off choice that takes effect when the form is sent."}>
        <Basic />
      </Lead>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Off"}>
            <StateOff />
          </Stage>
          <Stage caption={"State: On"}>
            <StateOn />
          </Stage>
          <Stage caption={"State: Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"State: Mixed"}>
            <StateMixed />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Terms on a sign-up form, with an error"}>
          <Controlled />
        </Stage>
        <Stage caption={"Saved by an action: busy while it runs, undone with a note if it fails"}>
          <Saved />
        </Stage>
      </PageSection>
    </>
  );
}
