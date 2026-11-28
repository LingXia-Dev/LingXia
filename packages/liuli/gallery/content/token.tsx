// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Recipients, Filters, Labels, Sizes, StateDisabled } from '../../core/src/Token/Token.examples';
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
        <Stage caption={"A hue for a category: the labels on an issue"}>
          <Labels />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small (24) and medium (32), plain and removable"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
