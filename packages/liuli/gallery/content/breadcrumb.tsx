// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Path, Deep, Pages } from '../../core/src/Breadcrumb/Breadcrumb.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a breadcrumb to show where a page sits in a hierarchy and to go back up it."}>
        <Path />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Six levels: the middle three fold into a menu"}>
          <Deep />
        </Stage>
        <Stage caption={"Levels that are pages of the app, as links"}>
          <Pages />
        </Stage>
      </PageSection>
    </>
  );
}
