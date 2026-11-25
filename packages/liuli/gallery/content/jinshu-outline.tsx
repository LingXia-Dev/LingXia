// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Progress, BesideTheDocument, InPopover } from '../../jinshu/src/JinshuOutline/JinshuOutline.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use JinshuOutline to let people see a long document's sections and move between them."} bare>
        <Progress />
      </Lead>
      <PageSection title={"In context"}>
        <Stage caption={"Beside the document"} bare>
          <BesideTheDocument />
        </Stage>
        <Stage caption={"Behind a Contents button"} bare>
          <InPopover />
        </Stage>
      </PageSection>
    </>
  );
}
