import { useLocal, useView, type View } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { Card } from '@liulidesign/core/Card';
import { CheckboxInput } from '@liulidesign/core/CheckboxInput';
import { FormLayout } from '@liulidesign/core/FormLayout';
import { Link } from '@liulidesign/core/Link';
import { TextInput } from '@liulidesign/core/TextInput';

type Actions = { signIn(): Promise<void> };

// Errors show only after the first attempt, then follow the typing until they are fixed.
function emailError(email: string) {
  if (email.trim() === '') return 'Enter your email.';
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email.trim())) return 'Enter an email like ana@example.com.';
  return undefined;
}

function passwordError(password: string) {
  if (password === '') return 'Enter your password.';
  if (password.length < 8) return 'Passwords here are at least 8 characters.';
  return undefined;
}

export function Content() {
  const { actions } = useView() as View<Actions>;
  const email = useLocal('email', '');
  const password = useLocal('password', '');
  const remember = useLocal('remember', true);
  const tried = useLocal('tried', false);
  const signedIn = useLocal('signedIn', false);
  const last = useLocal('last', '');

  const errors = { email: emailError(email.value), password: passwordError(password.value) };
  const status = (message: string | undefined) => (tried.value && message !== undefined ? { type: 'error' as const, message } : undefined);

  async function submit() {
    tried.set(true);
    if (errors.email !== undefined || errors.password !== undefined) return;
    await actions.signIn();
    signedIn.set(true);
  }

  if (signedIn.value)
    return (
      <>
        <Card padding="lg">
          <view className="gap-4">
            <view className="flex-row items-center justify-between gap-3">
              <text role="heading" headingLevel={1} className="text-title font-semibold tracking-tight text-text">You're in</text>
              <Badge label="Signed in" tone="success" data-testid="signed-in" />
            </view>
            <text className="text-body text-muted">{`Signed in as ${email.value.trim()}. Your notes are syncing.`}</text>
            <Button
              label="Sign out"
              width="content"
              onClick={() => { signedIn.set(false); tried.set(false); password.set(''); }}
              data-testid="sign-out"
            />
          </view>
        </Card>
      </>
    );

  return (
    <>
      <view className="gap-1.5 pt-2">
        <text role="heading" headingLevel={1} className="text-display font-semibold tracking-tight text-text">Welcome back</text>
        <text className="text-body text-muted">Sign in to sync your notes across devices.</text>
      </view>
      <FormLayout>
        <TextInput
          bind={email}
          label="Email"
          type="email"
          autoComplete="email"
          placeholder="ana@example.com"
          size="lg"
          isRequired
          hasRequiredMark={false}
          status={status(errors.email)}
          data-testid="email"
        />
        <TextInput
          bind={password}
          label="Password"
          type="password"
          size="lg"
          isRequired
          hasRequiredMark={false}
          description="At least 8 characters."
          status={status(errors.password)}
          enterKeyHint="go"
          data-testid="password"
        />
        <CheckboxInput
          value={remember.value}
          onChange={next => remember.set(next)}
          label="Remember me"
          size="lg"
          description="On this device only."
          data-testid="remember"
        />
      </FormLayout>
      <view className="gap-2">
        <Button label="Sign in" variant="primary" size="lg" width="fill" clickAction={submit} data-testid="submit" />
        <Button label="Forgot password?" variant="link" width="content" className="self-center" onClick={() => last.set('Sent a password reset link')} data-testid="forgot" />
      </view>
      <text className="text-center text-footnote text-muted">
        New to Notes? <Link label="Create an account" onClick={() => last.set('Opened Create an account')} data-testid="create" />
      </text>
      {last.value !== '' && <text role="status" className="text-center text-footnote text-muted">{last.value}</text>}
    </>
  );
}
