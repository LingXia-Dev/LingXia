// Generated from the doc file by scripts/generate.mjs; do not edit.
import { ExpandableSheet, FullScreenWithActions } from '../../core/src/WebView/WebView.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use WebView to embed a web page or map inline and expand it into a sheet or full screen without reloading."} bare>
        <ExpandableSheet />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Full-screen expansion with a footer action"} bare>
          <FullScreenWithActions />
        </Stage>
      </PageSection>
    </>
  );
}
