// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Weight, Area, Smooth, Series, Gaps, Goal, Sizes, StateSingle, StateLoading, StateEmpty, StateError, Selected } from '../../charts/src/LineChart/LineChart.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a line chart to show how a number moves over time."}>
        <Weight />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"An area: an amount, measured from zero"}>
          <Area />
        </Stage>
        <Stage caption={"Smoothed: through every reading, never beyond one"}>
          <Smooth />
        </Stage>
        <Stage caption={"Three series, told apart by dash and colour"}>
          <Series />
        </Stage>
        <Stage caption={"Missing readings stay gaps"}>
          <Gaps />
        </Stage>
        <Stage caption={"A goal across the plot, named with its value"}>
          <Goal />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small without labels, and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"Selected, loading, empty and error"} note={"What the chart shows in each state. These are staged examples: nothing here is waiting or broken."}>
        <StageGrid stack>
          <Stage caption={"With one value: a dot, since one point makes no line"}>
            <StateSingle />
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
      <PageSection title={"In context"}>
        <Stage caption={"Drag across the plot, or use the arrow keys, to read a month"}>
          <Selected />
        </Stage>
      </PageSection>
    </>
  );
}
