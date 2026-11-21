// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Results, Pages, Compact, Narrow, Cursor, PageSize } from '../../core/src/Pagination/Pagination.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use pagination under a table or list that is shown a page at a time."}>
        <Results />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Page numbers: first, last, the current page and its neighbours"}>
          <Pages />
        </Stage>
        <Stage caption={"The arrows and the position"}>
          <Compact />
        </Stage>
        <Stage caption={"Page numbers at 280 and 232 points: fewer neighbours, then the position"}>
          <Narrow />
        </Stage>
        <Stage caption={"A cursor: no count, Next stops where nothing follows"}>
          <Cursor />
        </Stage>
        <Stage caption={"Rows per page, keeping the first row in view"}>
          <PageSize />
        </Stage>
      </PageSection>
    </>
  );
}
