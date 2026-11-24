// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Standard, WithSearch } from '../../core/src/TopBar/TopBar.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use TopBar on a page that draws its own header: a way back, its title, a search field or a filter at the middle, its actions at the end."} bare>
        <Standard />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A search field at the middle"} bare>
          <WithSearch />
        </Stage>
      </PageSection>
    </>
  );
}
