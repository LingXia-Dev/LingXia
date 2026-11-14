// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Spending, Solid, Merged, Sizes, StateSelected, StateWhole, StateLoading, StateEmpty, StateError } from '../../../charts/src/PieChart/PieChart.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
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
      <PageSection title={"States"}>
        <StageGrid stack>
          <Stage caption={"One chosen"}>
            <StateSelected />
          </Stage>
          <Stage caption={"One part"}>
            <StateWhole />
          </Stage>
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
