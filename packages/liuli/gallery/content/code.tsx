// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Command, Plain, Capped, Wrapped, Numbered, Diff, InSentence } from '../../core/src/Code/Code.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use code for text that must be read or typed exactly."}>
        <Command />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"No header; long lines scroll sideways"}>
          <Plain />
        </Stage>
        <Stage caption={"A long file, capped at 160 points"}>
          <Capped />
        </Stage>
        <Stage caption={"Long lines wrapped at the width"}>
          <Wrapped />
        </Stage>
        <Stage caption={"Line numbers from 41, two lines marked"}>
          <Numbered />
        </Stage>
        <Stage caption={"A change, with signs and tints"}>
          <Diff />
        </Stage>
        <Stage caption={"Code, inside a sentence"}>
          <InSentence />
        </Stage>
      </PageSection>
    </>
  );
}
