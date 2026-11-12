// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Confirm, Form } from '../../../core/src/Dialog/Dialog.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"Confirm a destructive action, as an alert dialog"}>
        <Confirm />
      </Section>
      <Section title={"A short form"}>
        <Form />
      </Section>
    </GalleryPage>
  );
}
