// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, More, Value, Sizes, StateEnabled, StateDisabled, InList } from '../../../core/src/DropdownMenu/DropdownMenu.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a dropdown menu for a short list of actions that do not need to stay on screen."}>
        <Basic />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Icon only, at the end of a row"}>
          <More />
        </Stage>
        <Stage caption={"Showing the current choice"}>
          <Value />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large triggers"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Default"}>
            <StateEnabled />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A menu on every row"}>
          <InList />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
