// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Navigation, WithStatus, WithFacts, WithRemove, WithStop, RightToLeft, TitledLists, Links, Selection, Values, WithControl, Destructive, Swipe, Compact, StatePressable, StateSelected, StateDisabled, Disabled } from '../../core/src/Item/Item.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an item for one row of a list: a setting, a message, a file."} bare>
        <Navigation />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A status beside the name"} bare>
          <WithStatus />
        </Stage>
        <Stage caption={"Several facts under a name, each wrapping within itself"} bare>
          <WithFacts />
        </Stage>
        <Stage caption={"A row with a remove button of its own"} bare>
          <WithRemove />
        </Stage>
        <Stage caption={"An action that ends something at once, in danger"} bare>
          <WithStop />
        </Stage>
        <Stage caption={"Right to left: the row mirrors, the action ends it on the left"} bare>
          <RightToLeft />
        </Stage>
        <Stage caption={"A list title on the card's edge, as a Section heading"} bare>
          <TitledLists />
        </Stage>
        <Stage caption={"Rows that go to a page, each one link"} bare>
          <Links />
        </Stage>
        <Stage caption={"Choosing one, the chosen row selected"} bare>
          <Selection />
        </Stage>
        <Stage caption={"Read-only, with a value at the end"} bare>
          <Values />
        </Stage>
        <Stage caption={"Holding its own control"} bare>
          <WithControl />
        </Stage>
        <Stage caption={"Destructive, for a row that deletes"} bare>
          <Destructive />
        </Stage>
        <Stage caption={"Drag a row aside for its actions; the list has a heading and a line under it"} bare>
          <Swipe />
        </Stage>
        <Stage caption={"Compact rows under a heading"} bare>
          <Compact />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Pressable"}>
            <StatePressable />
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
        <Stage caption={"Disabled, with the reason"} bare>
          <Disabled />
        </Stage>
      </PageSection>
    </>
  );
}
