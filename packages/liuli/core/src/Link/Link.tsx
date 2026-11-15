import { linkRecipe as r } from './Link.recipe';

export interface LinkProps {
  /** The linked words. They say where the link goes: "terms of service", never "here". */
  label: string;
  /** Called when the link is pressed. The app opens the page or the address. */
  onClick?: () => void;
  /** Replaces the label for assistive tech when the words alone are not enough out of their sentence. */
  'aria-label'?: string;
  /** Takes no press and is drawn faint. @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

/** A link inside a sentence. It goes in a `<text>`, among its words. */
export function Link({ label, onClick, 'aria-label': ariaLabel, isDisabled = false, 'data-testid': testId }: LinkProps) {
  return (
    <link data-testid={testId} aria-label={ariaLabel} onClick={isDisabled ? undefined : onClick} className={isDisabled ? r.root.disabled : r.root.base}>
      {label}
    </link>
  );
}
