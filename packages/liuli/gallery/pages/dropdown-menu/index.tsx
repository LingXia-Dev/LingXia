import { Basic, More, InList } from '../../../core/src/DropdownMenu/DropdownMenu.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={'Labelled trigger'}>
        <Basic />
      </Section>
      <Section title={'Icon-only "more" menu on a row'}>
        <More />
      </Section>
      <Section title={'A menu on every row'}>
        <InList />
      </Section>
    </GalleryPage>
  );
}
