// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Hero, VariantsEveryday, VariantDestructive, VariantLink, WithIcons, Sizes, StateDefault, StateBusy, StateDisabled, StateSecondaryDisabled, DisabledWithReason, AsyncAction, Floating } from '../../core/src/Button/Button.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a button to start an action: save, send, delete or open a dialog. Keep one primary button per view."}>
        <Hero />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Primary for the main action, secondary and ghost for the rest"}>
          <VariantsEveryday />
        </Stage>
        <Stage caption={"Destructive, for loss"}>
          <VariantDestructive />
        </Stage>
        <Stage caption={"Link, a quiet way out beside a form"}>
          <VariantLink />
        </Stage>
        <Stage caption={"An icon before the label for the kind of action, after it for where it leads"}>
          <WithIcons />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Default"}>
            <StateDefault />
          </Stage>
          <Stage caption={"While busy"}>
            <StateBusy />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"Disabled, secondary"}>
            <StateSecondaryDisabled />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A form footer that waits for consent"}>
          <DisabledWithReason />
        </Stage>
        <Stage caption={"Waiting on a Logic action"}>
          <AsyncAction />
        </Stage>
        <Stage caption={"Floating: one main action over content that scrolls"}>
          <Floating />
        </Stage>
      </PageSection>
    </>
  );
}
