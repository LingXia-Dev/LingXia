// Generated from the doc file by scripts/generate.mjs; do not edit.
import { InSentence, InBody, Disabled } from '../../core/src/Link/Link.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a link for words in a sentence that lead to another page."}>
        <InSentence />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"At the size of the text around it"}>
          <InBody />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"Disabled"}>
            <Disabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
