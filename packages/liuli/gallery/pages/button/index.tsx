// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Variants, Sizes, AsyncAction, DisabledWithReason } from '../../../core/src/Button/Button.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"Variants"}>
        <Variants />
      </Section>
      <Section title={"Sizes"}>
        <Sizes />
      </Section>
      <Section title={"Waiting on a Logic action"}>
        <AsyncAction />
      </Section>
      <Section title={"Disabled, with the reason"}>
        <DisabledWithReason />
      </Section>
    </GalleryPage>
  );
}
