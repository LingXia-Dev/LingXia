// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Filters, Navigation } from '../../core/src/Drawer/Drawer.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a drawer for a panel of its own beside the page it came from."}>
        <Filters />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"From the start edge, narrow, for an app's places"}>
          <Navigation />
        </Stage>
      </PageSection>
    </>
  );
}
