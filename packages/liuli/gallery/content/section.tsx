// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Recent, Nested } from '../../core/src/Section/Section.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a section to give a part of a page its heading."} bare>
        <Recent />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Sections inside a section, their headings a level down"} bare>
          <Nested />
        </Stage>
      </PageSection>
    </>
  );
}
