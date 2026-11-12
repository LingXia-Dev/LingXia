// Accessibility patterns as data. A doc file names its pattern; a device spec walks the pattern's
// checks against every example through the semantics tree (`@lingxia/test` locators), never pixels.

export type PatternName = keyof typeof patterns;

/** Roles `@lingxia/test`'s `getByRole` resolves today. */
export type LocatorRole = 'button' | 'textbox' | 'checkbox' | 'switch' | 'dialog' | 'menu' | 'menuitem' | 'combobox' | 'heading' | 'status' | 'alert'
  | 'slider' | 'tablist' | 'tab' | 'tabpanel' | 'radiogroup' | 'radio';

export type Step =
  | { readonly do: 'find'; readonly role: LocatorRole; readonly name: 'label' | 'menu label' }
  | { readonly do: 'tab-to'; readonly role: LocatorRole }
  | { readonly do: 'press-key'; readonly key: 'enter' | 'space' | 'escape' | 'down' | 'up' | 'left' | 'right' | 'home' | 'end' | 'tab' }
  | { readonly do: 'tap'; readonly role: LocatorRole }
  | { readonly do: 'dismiss' }
  | { readonly expect: 'focused'; readonly role: LocatorRole; readonly which?: 'first' | 'last' | 'invoker' }
  | { readonly expect: 'name-stable-across-states' }
  | { readonly expect: 'attached' | 'detached'; readonly role: LocatorRole }
  | { readonly expect: 'value'; readonly value: string }
  | { readonly expect: 'checked'; readonly checked: boolean }
  | { readonly expect: 'disabled' | 'enabled'; readonly role: LocatorRole }
  | { readonly expect: 'handler-ran'; readonly times: number };

export interface Pattern {
  readonly role: LocatorRole;
  /** Checks every component on this pattern must pass, as ordered steps. */
  readonly checks: Readonly<Record<string, readonly Step[]>>;
  /** Checks that need a hardware keyboard; the runner skips them on touch-only targets. */
  readonly keyboardChecks?: Readonly<Record<string, readonly Step[]>>;
}

export const patterns = {
  button: {
    role: 'button',
    checks: {
      'is found by its label': [{ do: 'find', role: 'button', name: 'label' }],
      'a tap runs it once': [{ do: 'tap', role: 'button' }, { expect: 'handler-ran', times: 1 }],
      'keeps its name while loading': [{ expect: 'name-stable-across-states' }],
    },
    keyboardChecks: {
      'is reachable with Tab and runs with Enter and Space': [
        { do: 'tab-to', role: 'button' }, { expect: 'focused', role: 'button' },
        { do: 'press-key', key: 'enter' }, { expect: 'handler-ran', times: 1 },
        { do: 'press-key', key: 'space' }, { expect: 'handler-ran', times: 2 },
      ],
    },
  },
  checkbox: {
    role: 'checkbox',
    checks: {
      'is found by its label': [{ do: 'find', role: 'checkbox', name: 'label' }],
      'a tap toggles it': [{ do: 'tap', role: 'checkbox' }, { expect: 'checked', checked: true }, { do: 'tap', role: 'checkbox' }, { expect: 'checked', checked: false }],
      'keeps its name when checked': [{ expect: 'name-stable-across-states' }],
    },
    keyboardChecks: {
      'Space toggles it': [{ do: 'tab-to', role: 'checkbox' }, { do: 'press-key', key: 'space' }, { expect: 'checked', checked: true }],
    },
  },
  textbox: {
    role: 'textbox',
    checks: {
      'is found by its label, not its placeholder': [{ do: 'find', role: 'textbox', name: 'label' }],
    },
    keyboardChecks: {
      'is reachable with Tab': [{ do: 'tab-to', role: 'textbox' }, { expect: 'focused', role: 'textbox' }],
    },
  },
  dialog: {
    role: 'dialog',
    checks: {
      'is found by its label once open': [{ do: 'find', role: 'dialog', name: 'label' }],
      'a dismiss request closes it': [{ do: 'dismiss' }, { expect: 'detached', role: 'dialog' }],
    },
    keyboardChecks: {
      'opening moves focus in; Escape closes and returns focus': [
        { expect: 'focused', role: 'button', which: 'first' },
        { do: 'press-key', key: 'escape' }, { expect: 'detached', role: 'dialog' },
        { expect: 'focused', role: 'button', which: 'invoker' },
      ],
    },
  },
  menuButton: {
    role: 'button',
    checks: {
      'the trigger is found by its label': [{ do: 'find', role: 'button', name: 'label' }],
      'a tap opens the menu by its label': [{ do: 'tap', role: 'button' }, { do: 'find', role: 'menu', name: 'menu label' }],
      'an item closes the menu': [{ do: 'tap', role: 'menuitem' }, { expect: 'detached', role: 'menu' }],
    },
    keyboardChecks: {
      'Enter opens on the first item; arrows wrap; Escape returns to the trigger': [
        { do: 'tab-to', role: 'button' }, { do: 'press-key', key: 'enter' },
        { expect: 'focused', role: 'menuitem', which: 'first' },
        { do: 'press-key', key: 'up' }, { expect: 'focused', role: 'menuitem', which: 'last' },
        { do: 'press-key', key: 'escape' }, { expect: 'detached', role: 'menu' },
        { expect: 'focused', role: 'button', which: 'invoker' },
      ],
    },
  },
  slider: {
    role: 'slider',
    checks: {
      'is found by its label': [{ do: 'find', role: 'slider', name: 'label' }],
    },
    keyboardChecks: {
      'is reachable with Tab; Right and Up raise it, Home and End jump to the ends': [
        { do: 'tab-to', role: 'slider' }, { expect: 'focused', role: 'slider' },
        { do: 'press-key', key: 'right' }, { do: 'press-key', key: 'home' }, { do: 'press-key', key: 'end' },
      ],
    },
  },
  tabs: {
    role: 'tablist',
    checks: {
      'the list is found by its label': [{ do: 'find', role: 'tablist', name: 'label' }],
      'a tap on a tab chooses it once': [{ do: 'tap', role: 'tab' }, { expect: 'handler-ran', times: 1 }],
    },
    keyboardChecks: {
      'one Tab stop on the chosen tab; arrows choose on arrival': [
        { do: 'tab-to', role: 'tab' }, { expect: 'focused', role: 'tab' },
        { do: 'press-key', key: 'right' }, { expect: 'handler-ran', times: 1 },
        { do: 'press-key', key: 'home' }, { expect: 'handler-ran', times: 2 },
      ],
    },
  },
} as const satisfies Record<string, Pattern>;
