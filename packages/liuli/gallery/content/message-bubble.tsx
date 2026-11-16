// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Exchange, States, Reasoning } from '../../core/src/MessageBubble/MessageBubble.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a message bubble for each message of a conversation."}>
        <Exchange />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"On its way, sent, and not sent with a way to try again"}>
          <States />
        </Stage>
        <Stage caption={"MessageReasoning: how an assistant got to its answer, one line until opened; a spinner while it is still thinking"}>
          <Reasoning />
        </Stage>
      </PageSection>
    </>
  );
}
