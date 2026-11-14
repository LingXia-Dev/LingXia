// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Budget, Legends, Readout, StateLoading, StateEmpty, StateError } from '../../../charts/src/Chart/Chart.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use the chart frame around a plot of your own, so it reads and behaves like Liuli's charts."}>
        <Budget />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Squares for bars and slices, strokes for lines; the second can hide a series"}>
          <Legends />
        </Stage>
        <Stage caption={"The readout: the chosen datum in words and figures"}>
          <Readout />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid stack>
          <Stage caption={"Loading"}>
            <StateLoading />
          </Stage>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Error"}>
            <StateError />
          </Stage>
        </StageGrid>
      </PageSection>
    </GalleryPage>
  );
}
