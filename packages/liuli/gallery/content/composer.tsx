// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Reply, RowWithAttach, WithAttach, Attached, StateSending, StateDisabled } from '../../core/src/Composer/Composer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a composer to write and send a message."} bare>
        <Reply />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"With a way to attach: attach, text and send in one row"} bare>
          <RowWithAttach />
        </Stage>
        <Stage caption={"Past one line the text takes the full width, to five lines; the buttons go under it"} bare>
          <WithAttach />
        </Stage>
        <Stage caption={"What is attached sits inside, over the text"} bare>
          <Attached />
        </Stage>
        <Stage caption={"Sending: the button is busy, the draft stays"} bare>
          <StateSending />
        </Stage>
        <Stage caption={"Disabled, with the reason"} bare>
          <StateDisabled />
        </Stage>
      </PageSection>
    </>
  );
}
