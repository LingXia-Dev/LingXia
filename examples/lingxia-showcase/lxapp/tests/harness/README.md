# HTTP fixture

`http-fixture.mjs` is a dependency-free server for the transfer contracts:
deterministic bytes with a known digest, a trickled stream for progress and
cancel, a truncated response, arbitrary status codes, and a multipart endpoint
that echoes back the field names, filename, MIME type, and byte count it
received. Testing `lx.downloadFile` / `lx.uploadFile` against the public
internet imports its flakiness and cannot produce those failure modes at all.

```bash
node tests/harness/http-fixture.mjs --port 0   # prints the base URL
lxdev test tests/ --arg httpBase=http://127.0.0.1:<port>
```

## Reaching it from an lxapp

`is_domain_allowed` and the rebinding guard in
`crates/lingxia-logic/src/fs/network_security.rs` both reject non-public
addresses before the effective policy is consulted. Both relax for one case: a
dev session, on a host the grant allows — Showcase home gets `*` public
network, and a dev session then also allows loopback. A standalone Runner is a
guest: with no provider it is unrestricted (dev session then allows loopback);
an explicit grant still needs `*` or `127.0.0.1`. A
release build has no dev session, so a shipped app still cannot reach the
user's network.

That grants no new authority: a dev session already carries an automation
channel that evaluates arbitrary code in the Logic runtime. What it buys is a
deterministic fixture instead of the public internet.

## Known gaps

- **The response `Content-Type` is ignored.** `download_extension`
  (`crates/lingxia-logic/src/fs/download.rs`) maps `image/png` and friends, but
  `build_user_cache_download_request` (`crates/lingxia-transfer/src/download/
  manager.rs`) hardcodes `mime_type: None` and the manager only ever propagates
  the *caller's* hint, so that branch is dead. A download whose URL has no file
  extension fails with "download response requires Content-Type or a URL file
  extension" — an error that names a path which never works, reported as
  `E_NETWORK` / "Server error" when the server answered 200. Hence `/file/<name>`
  alongside `/bytes`.
- The Windows and Android runners do not start the fixture yet, so the transfer
  specs register as pending there.
