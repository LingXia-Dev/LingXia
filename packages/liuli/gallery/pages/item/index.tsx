// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Navigation, Values, WithControl, Disabled } from '../../../core/src/Item/Item.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"Rows that open a page"}>
        <Navigation />
      </Section>
      <Section title={"Read-only rows with values"}>
        <Values />
      </Section>
      <Section title={"A control at the end"}>
        <WithControl />
      </Section>
      <Section title={"Disabled, with the reason"}>
        <Disabled />
      </Section>
    </GalleryPage>
  );
}
