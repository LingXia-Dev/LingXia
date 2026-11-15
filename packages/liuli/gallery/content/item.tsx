// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Navigation, Selection, Values, WithControl, Destructive, Swipe, Compact, StatePressable, StateSelected, StateDisabled, Disabled } from '../../core/src/Item/Item.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an item for one row of a list: a setting, a message, a file."} bare>
        <Navigation />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Choosing one, the chosen row selected"} bare>
          <Selection />
        </Stage>
        <Stage caption={"Read-only, with a value at the end"} bare>
          <Values />
        </Stage>
        <Stage caption={"Holding its own control"} bare>
          <WithControl />
        </Stage>
        <Stage caption={"Danger tone, for a row that deletes"} bare>
          <Destructive />
        </Stage>
        <Stage caption={"Drag a row aside for its actions; the list has a heading and a line under it"} bare>
          <Swipe />
        </Stage>
        <Stage caption={"Compact rows under a heading"} bare>
          <Compact />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Pressable"} bare>
            <StatePressable />
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
        <Stage caption={"Disabled, with the reason"} bare>
          <Disabled />
        </Stage>
      </PageSection>
    </>
  );
}
