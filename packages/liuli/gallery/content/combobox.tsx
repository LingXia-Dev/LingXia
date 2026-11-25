// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Country, Clearable, Assignee, OwnMatching, Sizes, StateEmpty, StateChosen, StateInvalid, StateDisabled, Shipping } from '../../core/src/Combobox/Combobox.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a combobox to choose one option from a list too long to scan, by typing part of its name."}>
        <Country />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Clearable: the clear button takes the choice back"}>
          <Clearable />
        </Stage>
        <Stage caption={"A second line for options with similar names; one that cannot be chosen is dimmed"}>
          <Assignee />
        </Stage>
        <Stage caption={"The app matches by its own rule: a code finds its airport"}>
          <OwnMatching />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid stack>
          <Stage caption={"Nothing chosen"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Chosen"}>
            <StateChosen />
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
        <Stage caption={"In a form"}>
          <Shipping />
        </Stage>
      </PageSection>
    </>
  );
}
