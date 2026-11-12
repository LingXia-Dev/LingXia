// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Determinate, Indeterminate, Compact, Spinners } from '../../../core/src/ProgressBar/ProgressBar.examples';
import { GalleryPage, Section } from '../../shared/Section';

export function render() {
  return (
    <GalleryPage>
      <Section title={"An amount the app words"}>
        <Determinate />
      </Section>
      <Section title={"Running, amount unknown"}>
        <Indeterminate />
      </Section>
      <Section title={"Thin, under its own heading"}>
        <Compact />
      </Section>
      <Section title={"Spinner sizes"}>
        <Spinners />
      </Section>
    </GalleryPage>
  );
}
