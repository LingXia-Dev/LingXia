// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Budget, Spending, Legends, Readout, StateLoading, StateEmpty, StateError } from '../../charts/src/Chart/Chart.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use the chart frame around a plot of your own, so it reads and behaves like Liuli's charts."}>
        <Budget />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Eight parts: the five chart colours, then three of the theme's hues"}>
          <Spending />
        </Stage>
        <Stage caption={"Squares for bars and slices, strokes for lines; the second can hide a series"}>
          <Legends />
        </Stage>
        <Stage caption={"The readout: the chosen datum in words and figures"}>
          <Readout />
        </Stage>
      </PageSection>
      <PageSection title={"Loading, empty and error"} note={"What the chart shows in place of its plot. These are staged examples: nothing here is waiting or broken."}>
        <StageGrid stack>
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
