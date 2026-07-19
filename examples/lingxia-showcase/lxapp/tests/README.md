# Automation tests

- `api/` type-checks every public `lx` and returned-object member, verifies the
  concrete runtime surfaces, and adds behavior contracts by domain.
- `pages/` renders every configured page with meaningful content and adds
  stable `data-testid` flows.
- `flows/` covers user journeys spanning pages, native UI, or system dialogs.

Run the same page and flow entries against both `--framework react` and
`--framework vue`; tests must not branch on the selected framework.

Start the showcase with `lingxia dev -p macos --framework react` (substitute
`windows` and `vue` for the other matrix cells), then run
`npm run test:automation` from `lxapp/`.
