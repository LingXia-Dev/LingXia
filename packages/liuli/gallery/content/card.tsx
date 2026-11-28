// Generated from the doc file by scripts/generate.mjs; do not edit.
import { WithActions, Variants, Floating, ToPage, StateUnselected, StateSelected, StateDisabled, Choice, Several } from '../../core/src/Card/Card.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a card to group content that belongs together, and a clickable card when the whole card opens or chooses something."} bare>
        <WithActions />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Outlined, raised and sunken"} bare>
          <Variants />
        </Stage>
        <Stage caption={"Floating, over a map or a picture"} bare>
          <Floating />
        </Stage>
        <Stage caption={"Going to a page, with a Like button of its own"} bare>
          <ToPage />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Clickable"}>
            <StateUnselected />
          </Stage>
          <Stage caption={"State: Selected"}>
            <StateSelected />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Choosing a plan"} bare>
          <Choice />
        </Stage>
        <Stage caption={"Choosing topics to follow"} bare>
          <Several />
        </Stage>
      </PageSection>
    </>
  );
}
