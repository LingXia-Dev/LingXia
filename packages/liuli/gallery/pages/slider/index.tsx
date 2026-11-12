// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Volume, Stepped, InList, Sizes, StateDefault, StateDisabled, Position } from '../../../core/src/Slider/Slider.examples';
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
        <Stage caption={"As a row in a list, beside Item rows"}>
          <InList />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small for dense rows and media, medium by default"}>
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
        <Stage caption={"A position in a recording, with what has loaded"}>
          <Position />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
