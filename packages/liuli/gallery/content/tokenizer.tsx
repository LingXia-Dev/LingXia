// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Recipients, Tags, Empty, StateDisabled } from '../../core/src/Tokenizer/Tokenizer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a tokenizer to enter several words of the user's own."}>
        <Recipients />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Free tags, in the small size"}>
          <Tags />
        </Stage>
        <Stage caption={"Empty and required, with an error"}>
          <Empty />
        </Stage>
        <Stage caption={"Disabled, with the reason"}>
          <StateDisabled />
        </Stage>
      </PageSection>
    </>
  );
}
