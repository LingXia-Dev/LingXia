// Generated from the doc file by scripts/generate.mjs; do not edit.
import { WithActions, Variants, Floating, StateUnselected, StateSelected, StateDisabled, Choice } from '../../core/src/Card/Card.examples';
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
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Clickable"} bare>
            <StateUnselected />
          </Stage>
          <Stage caption={"Selected"} bare>
            <StateSelected />
          </Stage>
          <Stage caption={"Disabled"} bare>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Choosing a plan"} bare>
          <Choice />
        </Stage>
      </PageSection>
    </>
  );
}
