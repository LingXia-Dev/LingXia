// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Draft, AskAgent, ReadOnly } from '../../jinshu/src/JinshuEditor/JinshuEditor.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use JinshuEditor to let people, and the app's agent, edit a structured document in place."} bare>
        <Draft />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Handing blocks to the app's agent"} bare>
          <AskAgent />
        </Stage>
        <Stage caption={"Read-only"} bare>
          <ReadOnly />
        </Stage>
      </PageSection>
    </>
  );
}
