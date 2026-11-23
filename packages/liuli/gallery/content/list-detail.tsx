// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Orders } from '../../core/src/ListDetail/ListDetail.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use ListDetail when choosing a row in a list opens its record: side by side on a wide or unfolded screen, one over the other on a phone, with Back."} bare>
        <Orders />
      </Lead>
    </>
  );
}
