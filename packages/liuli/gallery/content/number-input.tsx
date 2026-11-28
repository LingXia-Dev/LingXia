// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Guests, Weight, Price, Sizes, StateEmpty, StateAtMinimum, StateInvalid, StateDisabled } from '../../core/src/NumberInput/NumberInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a number input for an amount people type or nudge: a count, a weight, a price."}>
        <Guests />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A unit after the number, and steps of a tenth"}>
          <Weight />
        </Stage>
        <Stage caption={"A typed number: a currency sign, no steppers, marked optional"}>
          <Price />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid stack>
          <Stage caption={"State: Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"State: At the minimum"}>
            <StateAtMinimum />
          </Stage>
          <Stage caption={"State: Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
