// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Confirm, Parts, Form, Full } from '../../core/src/Dialog/Dialog.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a dialog for a decision or a short task that must be finished before going on."}>
        <Confirm />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"Title, then the body, then the actions with the main one last"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"A short task: renaming"}>
          <Form />
        </Stage>
        <Stage caption={"Full screen, for a task with its own content"}>
          <Full />
        </Stage>
      </PageSection>
    </>
  );
}
