// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Revision, Inline, Block, NoChanges } from '../../jinshu/src/JinshuDiff/JinshuDiff.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use JinshuDiff to show what changed between two versions of a document."} bare>
        <Revision />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Inline"} bare>
          <Inline />
        </Stage>
        <Stage caption={"Block"} bare>
          <Block />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid stack>
          <Stage caption={"State: No changes"}>
            <NoChanges />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
