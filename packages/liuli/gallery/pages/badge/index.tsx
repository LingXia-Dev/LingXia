// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Tones, Solid, Sizes, InARow } from '../../../core/src/Badge/Badge.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"Soft tones"}>
        <Tones />
      </Section>
      <Section title={"Solid, to stand out"}>
        <Solid />
      </Section>
      <Section title={"Sizes and an icon"}>
        <Sizes />
      </Section>
      <Section title={"At the end of a row"}>
        <InARow />
      </Section>
    </GalleryPage>
  );
}
