# Testing GPU Views

Three levels: a View component in Node, the running page from the shell, and `lxdev test`
([Testing](../testing.md)) for behaviour that needs the app.

## In Node

A GPU View component is unit-tested in Node with `@lingxia/view/testing`: `render(page, view)`
compiles a page and runs it on the real View runtime, with a phone `env` you can override. Query by
test id, text or the props you wrote; every element event has a screen method (`press`, `type`,
`toggle`, `select`, `slide`, `escape`, …). Roles, names and layout need a device: use `lxdev test`.
Theme steps (`p-4`, `text-sm`) reach the wire as `[slot, times, plus]`; `points(props)` resolves them
under the built-in pack to compare with numbers, and `render(page, view, { points: true })` does so for every node.
A page using `t` needs `render(page, view, { messages: '.lingxia/i18n/build' })` after `lingxia build`; without it `t` throws `UI_I18N`.
A page's `useChannel` is served by `render(page, { channels: { feed(channel, params) { … } } })`: `channel.send`, `channel.on('data' | 'close')`, `channel.close`; `await screen.settle()` after a step lets the frames run.

```js
import { render, textOf } from '@lingxia/view/testing';
const screen = await render('tests/counter.page.tsx', { data: { label: 'Taps' } });
screen.press(screen.getByTestId('inc'));   // fires onClick, re-renders
assert.equal(textOf(screen.getByTestId('value')), 'Taps: 1');
```

## On the running page

```bash
lxdev lxapp page tree                          # role, name, test id, state, bounds in reading order
lxdev lxapp page find button --name "Save"     # nodes by role and whole name
lxdev lxapp page click '[data-testid="save"]'  # selectors match test ids, roles, labels, data-* props
lxdev lxapp page screenshot                    # only for a visual change
```

There is no DOM: `lxdev lxapp page eval` is for WebView pages. Logic state is `lxdev lxapp eval`.

## Locators

GPU-specific locator behaviour: `locator.dblclick()` selects the word at an element's centre in selectable
text and editors (a long press on touch); `locator.blur()` takes focus only from an editor that holds it.
The full locator API: [Testing → Common tasks](../testing.md#common-tasks).
