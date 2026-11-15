// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Note, Reference, Comment } from '../../core/src/Markdown/Markdown.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use Markdown to show text someone wrote: a message, a note, a help page."} bare>
        <Note />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Inline code, a code block, a table and a quote"} bare>
          <Reference />
        </Stage>
        <Stage caption={"Small, for a comment or a card"} bare>
          <Comment />
        </Stage>
      </PageSection>
    </>
  );
}
