# LingXia process-authority patch

This crate is copied from the published `rong_command` 0.6.0 crate
(`https://github.com/LingXia-Dev/Rong`, source commit
`4f9626b941804d9c89f45f1ce9d42bb756496821`, crates.io checksum
`fd408eb9651e9d910af00dfb426f7f518bbf46a736534067e6482df84e4d882f`). It is
published as the uniquely named `lingxia-rong-command` package so downstream
`lingxia-lxapp` builds receive the authority API without relying on a workspace
root `[patch.crates-io]` override.

The upstream Rong workspace declares `MIT OR Apache-2.0`. The published
`rong_command` manifest inherits that workspace license, and the root `rong`
package built from the same source commit contains `LICENSE-MIT` and
`LICENSE-APACHE`; those files are preserved here verbatim. That source release
does not contain a `NOTICE` file, so none is synthesized here.

LingXia's delta adds a sealed per-JavaScript-context `ProcessAuthority`. The
authority is checked before command parameters are decoded, at every retained
child/shell operation, and while synchronous or asynchronous children run.
Revocation terminates the child process tree. The remaining source matches the
published Rong crate so this package can be retired after the API is available
in a Rong release.
