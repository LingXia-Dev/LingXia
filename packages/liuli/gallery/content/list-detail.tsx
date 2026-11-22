// Generated from the doc file by scripts/generate.mjs; do not edit.
import { OrdersMasterDetail } from '../../core/src/ListDetail/ListDetail.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use ListDetail when choosing an item in a list opens its record: side-by-side on wide and unfolded screens, drill-down with Back support on phones."} bare>
        <OrdersMasterDetail />
      </Lead>
    </>
  );
}
