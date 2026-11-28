// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Filters, Layout, Single, Sizes, StateDisabled } from '../../core/src/ToggleButtonGroup/ToggleButtonGroup.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a toggle button group for toggles that belong together, with one on at a time or any number."}>
        <Filters />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"One on at a time, icon-only, read as a radio group"}>
          <Layout />
        </Stage>
        <Stage caption={"One on at a time, with labels"}>
          <Single />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
