// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Trend, Variants, Smooth, Tones, Sizes, StateMarked, StateFlat, StateSingle, StateEmpty, Gaps, InRows } from '../../charts/src/Sparkline/Sparkline.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a sparkline beside a figure to show which way it has been going."}>
        <Trend />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Line, area and bars"}>
          <Variants />
        </Stage>
        <Stage caption={"Smooth: rounded, never past its data"}>
          <Smooth />
        </Stage>
        <Stage caption={"Tones; positive and negative are claims about the data"}>
          <Tones />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small for rows, medium by default, large on its own"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Latest value marked"}>
            <StateMarked />
          </Stage>
          <Stage caption={"No change"}>
            <StateFlat />
          </Stage>
          <Stage caption={"One value"}>
            <StateSingle />
          </Stage>
          <Stage caption={"No values"}>
            <StateEmpty />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Missing readings stay gaps"}>
          <Gaps />
        </Stage>
        <Stage caption={"In list rows, between the name and the figure"}>
          <InRows />
        </Stage>
      </PageSection>
    </>
  );
}
