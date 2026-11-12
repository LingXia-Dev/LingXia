// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Basic, Controlled, Sizes } from '../../../core/src/CheckboxInput/CheckboxInput.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"With help text"}>
        <Basic />
      </Section>
      <Section title={"Owned by the app, with an error"}>
        <Controlled />
      </Section>
      <Section title={"Sizes"}>
        <Sizes />
      </Section>
    </GalleryPage>
  );
}
