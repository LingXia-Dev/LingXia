// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Volume, Stepped, Sizes, StateDefault, StateDisabled, Position } from '../../../core/src/Slider/Slider.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a slider when the value is easier to feel than to type: a volume, a size, a place in a recording."}>
        <Volume />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Whole steps, named in words"}>
          <Stepped />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Default"}>
            <StateDefault />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A position in a recording"}>
          <Position />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
