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

## Not reachable from an lxapp yet

`LxAppSecurity::is_domain_allowed` rejects loopback and private addresses
before `trustedDomains` is consulted (`crates/lingxia-lxapp/src/lxapp/
security.rs`, `is_public_network_address`). That rule is right — an lxapp has
no business reaching the host's local network — and it also blocks a fixture
on `127.0.0.1`, from Logic and from the page alike.

Closing that gap is a deliberate design decision, not a config change:

1. **A dev-session allowance.** While `lingxia dev` is live, the runtime trusts
   one fixture origin it was told about. Scoped to the dev broker, absent in a
   release build. Smallest change, and it keeps the production rule intact.
2. **A fixture served from a public host.** No runtime change, but the suite
   depends on a deployment and stops being self-contained.
3. **A loopback exception behind an explicit lxapp privilege.** Widest blast
   radius; it would weaken the rule for real apps too, so not recommended.

Until one of those lands, the transfer capabilities stay owned by
`PEND-UPLOAD-001` and `PEND-DOWNLOAD-001` rather than being quietly skipped.
