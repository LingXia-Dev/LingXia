// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Filters, Icons, Sizes, StateOff, StateOn, StateDisabled, StateDisabledOn, WordWrap } from '../../core/src/ToggleButton/ToggleButton.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a toggle button to turn one option on or off right where it applies."}>
        <Filters />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"An icon beside the label, an icon that changes when on, and icon-only"}>
          <Icons />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large, on"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Off"}>
            <StateOff />
          </Stage>
          <Stage caption={"On"}>
            <StateOn />
          </Stage>
          <Stage caption={"Disabled, off"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"Disabled, on"}>
            <StateDisabledOn />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A view option beside what it changes"}>
          <WordWrap />
        </Stage>
      </PageSection>
    </>
  );
}
