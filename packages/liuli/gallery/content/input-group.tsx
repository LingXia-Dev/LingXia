// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Price, Website, Invite, Sizes } from '../../core/src/InputGroup/InputGroup.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an input group when a field needs fixed text, a choice or an action joined to it to make one value."}>
        <Price />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Fixed text before the field"}>
          <Website />
        </Stage>
        <Stage caption={"A field and its action"}>
          <Invite />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"The group sets the size of its controls"}>
          <Sizes />
        </Stage>
      </PageSection>
    </>
  );
}
