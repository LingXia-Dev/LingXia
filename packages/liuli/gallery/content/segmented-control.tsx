// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Units, Sizes, StateTwo, StateFive, StateDisabled, Appearance } from '../../core/src/SegmentedControl/SegmentedControl.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a segmented control to set one value from a few short choices, applied at once."}>
        <Units />
      </Lead>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Two choices"}>
            <StateTwo />
          </Stage>
          <Stage caption={"Five choices"}>
            <StateFive />
          </Stage>
          <Stage caption={"A disabled segment"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"At the end of a setting row"}>
          <Appearance />
        </Stage>
      </PageSection>
    </>
  );
}
