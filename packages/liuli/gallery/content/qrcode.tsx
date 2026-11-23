// Generated from the doc file by scripts/generate.mjs; do not edit.
import { Join, Link, Sizes } from '../../core/src/QRCode/QRCode.examples';
import { Lead, PageSection, Stage } from '../shared/Layout';

export function Content() {
  return (
    <>
      <Lead text={"Use a QR code to hand a link or a code to a phone held up to the screen."}>
        <Join />
      </Lead>
      <PageSection title={"Variants"}>
        <Stage caption={"A link, with the address under it"}>
          <Link />
        </Stage>
        <Stage caption={"Small, medium and large"}>
          <Sizes />
        </Stage>
      </PageSection>
    </>
  );
}
