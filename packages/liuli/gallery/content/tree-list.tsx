// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Folders, Outline, Guides, Described } from '../../core/src/TreeList/TreeList.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a tree for things inside things: folders, an outline."}>
        <Folders />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Without icons, three levels deep"}>
          <Outline />
        </Stage>
        <Stage caption={"A hairline down each open branch"}>
          <Guides />
        </Stage>
        <Stage caption={"A second line, a status and a button at the end"}>
          <Described />
        </Stage>
      </PageSection>
    </>
  );
}
