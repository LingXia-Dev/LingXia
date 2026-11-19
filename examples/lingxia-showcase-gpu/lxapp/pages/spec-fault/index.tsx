import '../../theme/index.css';

// For specs only: every render throws, so the page shows the recovery panel ("This page couldn't
// be displayed.", Retry, and Go back when there is a page to return to). Retry fails again.
export function render(): never {
  throw new Error('spec-fault: this page fails on purpose');
}
