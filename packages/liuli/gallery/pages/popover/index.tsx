// Generated from the doc file by scripts/generate.mjs; do not edit.
import { ViewOptions, Parts, Share } from '../../../core/src/Popover/Popover.examples';
import { GalleryPage, Lead, PageSection, Stage } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a popover for a few controls or lines that belong to the button that opens them."}>
        <ViewOptions />
      </Lead>
      <PageSection title="Parts">
        <Stage caption={"The trigger, then the panel with its title"} bare>
          <Parts />
        </Stage>
      </PageSection>
      <PageSection title={"Variants"}>
        <Stage caption={"Above, lined up with the trigger's end"}>
          <Share />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
