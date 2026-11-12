import { WrappedSelect } from '../../../core/src/Field/Field.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={'Wrapping a primitive select'}>
        <WrappedSelect />
      </Section>
    </GalleryPage>
  );
}
