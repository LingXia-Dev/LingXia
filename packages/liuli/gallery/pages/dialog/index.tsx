import { Confirm, Form } from '../../../core/src/Dialog/Dialog.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={'Confirm a destructive action'}>
        <Confirm />
      </Section>
      <Section title={'A short form'}>
        <Form />
      </Section>
    </GalleryPage>
  );
}
