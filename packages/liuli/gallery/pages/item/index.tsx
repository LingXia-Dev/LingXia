// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Navigation, Values, WithControl, Destructive, StatePressable, StateDisabled, Disabled } from '../../../core/src/Item/Item.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use an item for one row of a list: a setting, a message, a file."} bare>
        <Navigation />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Read-only, with a value at the end"} bare>
          <Values />
        </Stage>
        <Stage caption={"Holding its own control"} bare>
          <WithControl />
        </Stage>
        <Stage caption={"Danger tone, for a row that deletes"} bare>
          <Destructive />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Pressable"} bare>
            <StatePressable />
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
    </GalleryPage>
  );
}
