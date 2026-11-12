// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Variants, WithActions, Choice } from '../../../core/src/Card/Card.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"Outlined, raised and sunken"}>
        <Variants />
      </Section>
      <Section title={"A card with actions"}>
        <WithActions />
      </Section>
      <Section title={"Choosing one card of several"}>
        <Choice />
      </Section>
    </GalleryPage>
  );
}
