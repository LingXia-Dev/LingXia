// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Exchange, States } from '../../core/src/MessageBubble/MessageBubble.examples';
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
      </PageSection>
    </>
  );
}
