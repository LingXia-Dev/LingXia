// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Address, Profile } from '../../core/src/FormLayout/FormLayout.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a form layout to set a form's fields at one rhythm and put its buttons after them."}>
        <Address />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Where most fields are optional, the required one is starred"}>
          <Profile />
        </Stage>
      </PageSection>
    </>
  );
}
