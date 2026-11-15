// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Invoice, Stacked } from '../../core/src/MetadataList/MetadataList.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a metadata list for the facts about one record."}>
        <Invoice />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Each label over its value, two to a line"}>
          <Stacked />
        </Stage>
      </PageSection>
    </>
  );
}
