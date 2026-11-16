import { useLocal } from '@lingxia/view';
import { Said, unused } from '../example';
import { Link } from './Link';

/** In a sentence, under a form. */
export function InSentence() {
  const opened = useLocal('opened', '');
  return (
    <view className="gap-2">
      <text className="text-footnote text-muted">
        By creating an account you accept the <Link label="terms of service" onClick={() => opened.set('terms')} data-testid="terms" /> and the{' '}
        <Link label="privacy notice" onClick={() => opened.set('privacy')} data-testid="privacy" />.
      </text>
      {opened.value !== '' && <text role="status" data-testid="opened" className="text-caption text-muted">{`Opened: ${opened.value}`}</text>}
    </view>
  );
}

/** In body text, at its size and weight. */
export function InBody() {
  return (
    <Said>
      {say => (
        <text className="text-body text-text">
          Exports are kept for seven days. You can <Link label="download the last export" onClick={() => say('Downloaded the last export')} data-testid="download" /> or start a new one from Settings.
        </text>
      )}
    </Said>
  );
}

/** A link that cannot be followed yet. */
export function Disabled() {
  return (
    <text className="text-footnote text-muted">
      No code yet? <Link label="Send it again" isDisabled data-testid="resend" /> in 30 seconds.
    </text>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <text className="text-footnote text-muted">
      Read the <Link label="guide" onClick={unused} />
    </text>
  );
}
