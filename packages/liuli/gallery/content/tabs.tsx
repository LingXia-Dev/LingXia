// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Sections, Segmented, StateUnderline, StateSegmented, StateDisabled, StateSegmentedDisabled, Filter } from '../../core/src/Tabs/Tabs.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use tabs to switch one area between a few peers. Each tab is a place, not an action."}>
        <Sections />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Segmented: modes of one view"}>
          <Segmented />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Underline"}>
            <StateUnderline />
          </Stage>
          <Stage caption={"Segmented"}>
            <StateSegmented />
          </Stage>
          <Stage caption={"A disabled tab"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"A disabled segment"}>
            <StateSegmentedDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Filtering a list"}>
          <Filter />
        </Stage>
      </PageSection>
    </>
  );
}
