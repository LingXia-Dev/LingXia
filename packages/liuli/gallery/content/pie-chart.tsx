// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Spending, Solid, Merged, Sizes, StateSelected, StateWhole, StateLoading, StateEmpty, StateError } from '../../charts/src/PieChart/PieChart.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a pie chart to show how one total divides into a few parts."}>
        <Spending />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A solid pie, when no total is needed"}>
          <Solid />
        </Stage>
        <Stage caption={"Eight parts: four keep a slice, the rest share a neutral one"}>
          <Merged />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and large, the legend left out"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"Selected, loading, empty and error"} note={"What the chart shows in each state. These are staged examples: nothing here is waiting or broken."}>
        <StageGrid stack>
          <Stage caption={"State: One chosen"}>
            <StateSelected />
          </Stage>
          <Stage caption={"State: One part"}>
            <StateWhole />
          </Stage>
          <Stage caption={"State: While loading"}>
            <StateLoading />
          </Stage>
          <Stage caption={"State: With no data"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"State: After a failure"}>
            <StateError />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
