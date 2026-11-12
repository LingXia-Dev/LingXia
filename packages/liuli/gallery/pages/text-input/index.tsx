import { Bound, Search, Disabled } from '../../../core/src/TextInput/TextInput.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={'Bound to local state, with validation'}>
        <Bound />
      </Section>
      <Section title={'Search with a clear button'}>
        <Search />
      </Section>
      <Section title={'Disabled, with the reason'}>
        <Disabled />
      </Section>
    </GalleryPage>
  );
}
