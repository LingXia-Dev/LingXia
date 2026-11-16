// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Storage, Parts, Below, Beside, Sizes, StateClosed, StatePressed, StateOpen, InForm } from '../../core/src/Tooltip/Tooltip.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a tooltip for a short note that explains what is beside it. Tap the info button to open it."}>
        <Storage />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The note over its button"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"Opening below, where there is no room above"}>
          <Below />
        </Stage>
        <Stage caption={"At the side of the button"}>
          <Beside />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Closed"}>
            <StateClosed />
          </Stage>
          <Stage caption={"Pressed"}>
            <StatePressed />
          </Stage>
          <Stage caption={"Open"}>
            <StateOpen />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Next to a field label"}>
          <InForm />
        </Stage>
      </PageSection>
    </>
  );
}
