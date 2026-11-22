import { registerPressable, useTrigger } from '../internal/trigger';
import { linkRecipe as r } from './Link.recipe';
import { dataOf } from '../internal/data';

export interface LinkProps {
  /** The linked words. They say where the link goes: "terms of service", never "here". */
  label: string;
  /** Called when the link is pressed. The app opens the page or the address. */
  onClick?: () => void;
  /** Takes no press, is drawn faint and is read as unavailable. @default false */
  isDisabled?: boolean;
  'data-testid'?: string;
}

/** A link inside a sentence. It goes in a `<text>`, among its words. */
export function Link({ label, onClick, isDisabled = false, 'data-testid': testId, ...attributes }: LinkProps) {
  const trigger = useTrigger();
  return (
    <link data-testid={testId} {...dataOf(attributes)} popoverTarget={trigger?.popoverTarget} interestFor={trigger?.interestFor} aria-describedby={trigger?.describedBy} disabled={isDisabled || undefined} onClick={onClick} className={isDisabled ? r.root.disabled : r.root.base}>
      {label}
    </link>
  );
}

registerPressable(Link);
