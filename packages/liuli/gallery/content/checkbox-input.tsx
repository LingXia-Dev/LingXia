// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, Sizes, StateOff, StateOn, StateInvalid, StateDisabled, Controlled } from '../../core/src/CheckboxInput/CheckboxInput.examples';
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
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Off"}>
            <StateOff />
          </Stage>
          <Stage caption={"On"}>
            <StateOn />
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
        <Stage caption={"Terms on a sign-up form, with an error"}>
          <Controlled />
        </Stage>
      </PageSection>
    </>
  );
}
