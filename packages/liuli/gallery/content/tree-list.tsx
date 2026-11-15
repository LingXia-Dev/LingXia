// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Folders, Outline } from '../../core/src/TreeList/TreeList.examples';
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
      </PageSection>
    </>
  );
}
