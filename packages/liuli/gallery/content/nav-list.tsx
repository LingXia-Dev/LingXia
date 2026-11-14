// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Sidebar, Closed } from '../../core/src/NavList/NavList.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a nav list for the column of places beside a workspace."}>
        <Sidebar />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A section that starts closed"}>
          <Closed />
        </Stage>
      </PageSection>
    </>
  );
}
