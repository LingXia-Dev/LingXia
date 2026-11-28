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
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"State: With help"}>
            <StateHelp />
          </Stage>
          <Stage caption={"State: Warning"}>
            <StateWarning />
          </Stage>
          <Stage caption={"State: With an error"}>
            <StateError />
          </Stage>
          <Stage caption={"State: Disabled"}>
            <StateDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
    </>
  );
}
