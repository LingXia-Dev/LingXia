// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Documents, Empty, StateDisabled } from '../../core/src/FileInput/FileInput.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a file input to collect files with a form."}>
        <Documents />
      </Lead>
      <PageSection title={"Variants"}>
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
