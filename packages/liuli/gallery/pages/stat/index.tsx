// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Revenue, Plain, Tones, Sizes, StateUp, StateDown, StateFlat, StateLoading, Dashboard } from '../../../charts/src/Stat/Stat.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a stat for a number people come back to check, with how it changed since last time."}>
        <Revenue />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A figure alone"}>
          <Plain />
        </Stage>
        <Stage caption={"Down can be good news and up bad: the tone is set apart from the direction"}>
          <Tones />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small for cards two to a row, medium by default"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Up"}>
            <StateUp />
          </Stage>
          <Stage caption={"Down"}>
            <StateDown />
          </Stage>
          <Stage caption={"No change"}>
            <StateFlat />
          </Stage>
          <Stage caption={"Loading"}>
            <StateLoading />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Four figures on a phone, two to a row"}>
          <Dashboard />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
