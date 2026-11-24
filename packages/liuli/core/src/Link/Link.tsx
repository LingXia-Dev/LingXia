import type { Press } from '../internal/press';
import { registerPressable, identityOf, useTrigger } from '../internal/trigger';
import { linkRecipe as r } from './Link.recipe';

export interface LinkOwnProps {
  /** The linked words. They say where the link goes: "terms of service", never "here". */
  label: string;
  /** Takes no press, is drawn faint and is read as unavailable. @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

export interface LinkActions {
  /** Called when the link is pressed: the app opens the page or the address. */
  onClick?: () => void;
}

export type LinkProps = LinkOwnProps & Press<LinkActions>;

/** A link inside a sentence. It goes in a `<text>`, among its words. */
export function Link({ label, onClick, to, isDisabled = false, 'data-testid': testId, ...attributes }: LinkProps) {
  const trigger = useTrigger();
  if (to !== undefined && (onClick !== undefined || trigger)) throw Error(`Link "${label}": give to or an action, not both`);
  return (
    <link {...identityOf(testId, attributes, trigger)} to={to} popoverTarget={trigger?.popoverTarget} interestFor={trigger?.interestFor} aria-describedby={trigger?.describedBy} disabled={isDisabled || undefined} onClick={onClick} className={isDisabled ? r.root.disabled : r.root.base}>
      {label}
    </link>
  );
}

registerPressable(Link);
