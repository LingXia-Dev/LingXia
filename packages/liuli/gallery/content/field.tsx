// Generated from the doc file by scripts/generate.mjs; do not edit.
import { WrappedSelect, Info, StateHelp, StateWarning, StateError, StateDisabled } from '../../core/src/Field/Field.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a field to give a control Liuli's label, help text and status when Liuli has no component for it yet."}>
        <WrappedSelect />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A note behind an info button beside the label"}>
          <Info />
        </Stage>
      </PageSection>
      <PageSection title={"States"}>
        <StageGrid>
          <Stage caption={"With help"}>
            <StateHelp />
          </Stage>
          <Stage caption={"Warning"}>
            <StateWarning />
          </Stage>
          <Stage caption={"Error"}>
            <StateError />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
