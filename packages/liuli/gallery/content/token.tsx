// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Recipients, Filters, Sizes, StateDisabled } from '../../core/src/Token/Token.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a token for one thing someone chose: a recipient, a tag, a filter."}>
        <Recipients />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Pressable, as filters that toggle"}>
          <Filters />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small (24) and medium (32), plain and removable"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
