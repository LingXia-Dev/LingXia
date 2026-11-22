// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Determinate, Indeterminate, Transfer, Spinners, LoadingMessage, Tones, SpinnerTones, SizesBar, StateStarted, StateHalf, StateDone, StateUnknown, Compact } from '../../core/src/ProgressBar/ProgressBar.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a progress bar for a task that takes a while, and a spinner for a short wait."}>
        <Determinate />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Amount unknown"}>
          <Indeterminate />
        </Stage>
        <Stage caption={"A transfer's progress"}>
          <Transfer />
        </Stage>
        <Stage caption={"Spinner, for a short wait"}>
          <Spinners />
        </Stage>
        <Stage caption={"With its label shown, for a panel or a page that is loading"}>
          <LoadingMessage />
        </Stage>
        <Stage caption={"A tone for a meter: reached, nearly full, over"}>
          <Tones />
        </Stage>
        <Stage caption={"Extra large over media; muted beside quiet text"}>
          <SpinnerTones />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium tracks"}>
          <SizesBar />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Started"}>
            <StateStarted />
          </Stage>
          <Stage caption={"Halfway"}>
            <StateHalf />
          </Stage>
          <Stage caption={"Done"}>
            <StateDone />
          </Stage>
          <Stage caption={"Unknown"}>
            <StateUnknown />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"Thin, under its own heading"}>
          <Compact />
        </Stage>
      </PageSection>
    </>
  );
}
