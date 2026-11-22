// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Bound, Plain, Search, Adornments, SignIn, ShareLink, Sizes, StateEmpty, StateFilled, StateInvalid, StateDisabled, Checking, ReadOnly, Disabled } from '../../core/src/TextInput/TextInput.examples';
import { Lead, PageSection, Stage, StageGrid } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a text input for one line the person types: a name, an email, a code or a search."}>
        <Bound />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Label and placeholder"}>
          <Plain />
        </Stage>
        <Stage caption={"Search, with a clear button"}>
          <Search />
        </Stage>
        <Stage caption={"A currency sign, a web address start and a unit; the last field is marked optional"}>
          <Adornments />
        </Stage>
        <Stage caption={"Signing in: the password field shows its text on request"}>
          <SignIn />
        </Stage>
        <Stage caption={"A copy button inside the box"}>
          <ShareLink />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
      <PageSection title={"States"} note={"Each state is set by its props so you can see it. Nothing here is waiting, missing or broken."}>
        <StageGrid>
          <Stage caption={"Empty"}>
            <StateEmpty />
          </Stage>
          <Stage caption={"Filled"}>
            <StateFilled />
          </Stage>
          <Stage caption={"Invalid"}>
            <StateInvalid />
          </Stage>
          <Stage caption={"Disabled"}>
            <StateDisabled />
          </Stage>
          <Stage caption={"While checking"}>
            <Checking />
          </Stage>
          <Stage caption={"Read-only"}>
            <ReadOnly />
          </Stage>
        </StageGrid>
      </PageSection>
      <PageSection title={"In context"}>
        <Stage caption={"A fixed value, with the reason"}>
          <Disabled />
        </Stage>
      </PageSection>
    </>
  );
}
