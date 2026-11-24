// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Orders, Grouped, Signed, Month, Target, Sizes, StateSelected, StateLoading, StateEmpty, StateError } from '../../charts/src/BarChart/BarChart.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a bar chart to compare amounts across a few categories."}>
        <Orders />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Two series side by side; the legend shows and hides each"}>
          <Grouped />
        </Stage>
        <Stage caption={"Above and below zero, on one line"}>
          <Signed />
        </Stage>
        <Stage caption={"Thirty bars: narrower, with the labels that fit"}>
          <Month />
        </Stage>
        <Stage caption={"A target and an average, their labels clear of the figures"}>
          <Target />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small without labels, and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"Selected, loading, empty and error"} note={"What the chart shows in each state. These are staged examples: nothing here is waiting or broken."}>
        <StageGrid stack>
          <Stage caption={"One chosen"}>
            <StateSelected />
          </Stage>
          <Stage caption={"While loading"}>
            <StateLoading />
          </Stage>
          <Stage caption={"With no data"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"After a failure"}>
            <StateError />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
