// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Actions, Tags, FromStart } from '../../core/src/OverflowList/OverflowList.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use an overflow list when a line of tags or actions must keep to one row at any width."}>
        <Actions />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Tags, then a count"}>
          <Tags />
        </Stage>
        <Stage caption={"Folding from the start of a path"}>
          <FromStart />
        </Stage>
      </PageSection>
    </>
  );
}
