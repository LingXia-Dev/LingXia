// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Bound, Plain, Search, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, Disabled } from '../../../core/src/TextInput/TextInput.examples';
import { GalleryPage, Lead, PageSection, Stage, StageGrid } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a text input for one line the person types: a name, an email, a code or a search."}>
        <Bound />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Label and placeholder"}>
          <Plain />
        </Stage>
        <Stage caption={"Search, with a clear button"}>
          <Search />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Filled"}>
            <StateFilled />
          </Stage>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A fixed value, with the reason"}>
          <Disabled />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
