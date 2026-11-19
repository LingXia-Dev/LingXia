// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Person, Term, InSentence } from '../../core/src/HoverCard/HoverCard.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a hover card to preview what a word names without leaving the page."}>
        <Person />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Beside the trigger, for a term in small text"}>
          <Term />
        </Stage>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"On a link in a sentence"}>
          <InSentence />
        </Stage>
      </PageSection>
    </>
  );
}
