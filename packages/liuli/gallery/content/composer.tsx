// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Reply, WithAttach, StateSending, StateDisabled } from '../../core/src/Composer/Composer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a composer to write and send a message."}>
        <Reply />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A long draft grows the field to five lines; the buttons stay on the last line"}>
          <WithAttach />
        </Stage>
        <Stage caption={"Sending: the button is busy, the draft stays"}>
          <StateSending />
        </Stage>
        <Stage caption={"Disabled, with the reason"}>
          <StateDisabled />
        </Stage>
      </PageSection>
    </>
  );
}
