// Generated from the doc file by scripts/generate.mjs; do not edit.
import { InARow, Tones, Solid, Sizes } from '../../../core/src/Badge/Badge.examples';
import { GalleryPage, Lead, PageSection, Stage } from '../../shared/Layout';

export function render() {
  return (
    <GalleryPage>
      <Lead text={"Use a badge to mark the status or kind of the thing beside it, in a word or two."}>
        <InARow />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"Soft, in five tones"}>
          <Tones />
        </Stage>
        <Stage caption={"Solid, for the one label that must stand out"}>
          <Solid />
        </Stage>
      </PageSection>
      <PageSection title={"Sizes"}>
        <Stage caption={"Small and medium, with an icon"}>
          <Sizes />
        </Stage>
      </PageSection>
    </GalleryPage>
  );
}
