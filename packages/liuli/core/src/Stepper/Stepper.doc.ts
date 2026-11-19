import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Stepper',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Where someone is in a task with a known number of steps',
  description:
    'Stepper shows the steps of a task that must be done in order: a checkout, a sign-up, an import. Finished steps carry a tick, the step in progress is ringed, and what is ahead is quiet. ' +
    'With `onChange` a finished step is a button that goes back to it. The app holds the step and shows its content.',
  keywords: ['stepper', 'steps', 'wizard', 'progress steps', 'multi-step', 'checkout', 'onboarding'],
  propsType: 'StepperProps',
  otherPropsTypes: ['StepperStep'],
  recipe: { map: 'stepperRecipe', parts: 'stepperParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The steps, a list named by `label`.' },
    { part: 'step', required: true, description: 'One step; a `button` when it is finished and `onChange` is set.' },
    { part: 'marker', required: true, description: 'A 24 px circle: a tick when done, the step\'s number otherwise.' },
    { part: 'line', required: false, description: 'Joins a step to the next; filled once the step is done.' },
    { part: 'label', required: true, description: 'The step\'s name; heavier on the step in progress.' },
    { part: 'description', required: false, description: 'A second line under the label; vertical only.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Through the finished steps, when they can be pressed' },
      { keys: 'Enter or Space', effect: 'Goes back to the step' },
    ],
    target: {},
    contrast: [
      { what: 'tick on a finished step', pair: ['on-accent-solid', 'accent-solid'], minimum: '3:1' },
      { what: 'number and ring of the step in progress', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'ring of a step ahead', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'label of a step ahead, numbers and second lines', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The steps are a list, so a screen reader says how many there are and where each stands.',
      'Each step is read as its name and its state in the app\'s words: "Address, Current step". The markers and lines are not read.',
      'State is carried by the marker\'s shape (tick, ring, thin ring) and the label\'s weight, not by colour alone.',
      'A vertical step that can be pressed is as tall as its two lines, about 44 px; a horizontal one is smaller, so offer a Back button as well.',
    ],
  },
  guidance: {
    do: [
      'Use it when the steps are known up front and done in order.',
      'Use `vertical` on a phone and wherever steps have descriptions; `horizontal` for up to four short labels on a wide form.',
      'Name steps by what is done in them: "Address", not "Step 2".',
      'Set `current` to the number of steps when the task is finished, so every step is ticked.',
    ],
    dont: [
      'Use it to move between pages that can be visited in any order; that is Tabs.',
      'Let a step ahead be pressed; validate the current step, then move on.',
      'Use it for a single progress value; that is a ProgressBar.',
    ],
  },
  examples: [
    { name: 'Checkout', title: 'A task in four steps' },
    { name: 'Across', title: 'Across the top of a form' },
  ],
  gaps: [
    { missing: 'A step marked as failed or optional.', needs: 'nothing from the View', today: 'Done, in progress or ahead.' },
    { missing: 'Horizontal steps that fold to "2 of 4" where they do not fit.', needs: 'nothing from the View', today: 'Labels are cut short; use `vertical` on a phone.' },
  ],
  related: ['ProgressBar', 'Tabs', 'Breadcrumb'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Steps of a task',
    order: 5,
    page: {
      lead: 'Use a stepper for a task with a known number of steps done in order.',
      hero: 'Checkout',
      variants: [{ name: 'Across', title: 'Horizontal, for up to four short labels' }],
    },
  },
};
