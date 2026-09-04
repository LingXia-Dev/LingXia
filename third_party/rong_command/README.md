# rong_command

Command execution APIs mounted on the `Rong` namespace.

## JS APIs

- `Rong.spawn(...)` - async subprocess wrapper with streams, timeouts, and exit hooks
- `Rong.spawnSync(...)` - synchronous subprocess execution with captured `stdout` / `stderr`
- `Rong.stdin` / `Rong.stdout` / `Rong.stderr` - runtime stdio handles on the `Rong` namespace
- `Rong.$` - shell template tag with `.text()`, `.json()`, `.lines()`, `.blob()`, `.run()`, `.quiet()`, `.nothrow()`, and `.cwd()`
