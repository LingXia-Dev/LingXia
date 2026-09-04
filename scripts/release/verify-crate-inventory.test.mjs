import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const verifier = fileURLToPath(new URL("./verify-crate-inventory.mjs", import.meta.url));

function metadata(packages) {
  return JSON.stringify({
    packages,
    workspace_members: packages.map((pkg) => pkg.id),
  });
}

function pkg(name, publish = null, directory = "crates") {
  return {
    id: `${name} 0.14.0 (path+file:///repo/${directory}/${name})`,
    name,
    publish,
    manifest_path: `/repo/${directory}/${name}/Cargo.toml`,
  };
}

function verify(packages, releaseNames) {
  return spawnSync(process.execPath, [verifier, ...releaseNames], {
    encoding: "utf8",
    input: metadata(packages),
  });
}

test("accepts every crates.io package regardless of workspace directory", () => {
  const result = verify(
    [pkg("lingxia"), pkg("lingxia-rong-command", null, "third_party")],
    ["lingxia", "lingxia-rong-command"],
  );
  assert.equal(result.status, 0, result.stderr);
});

test("fails when a publishable workspace package is omitted", () => {
  const result = verify(
    [pkg("lingxia"), pkg("lingxia-rong-command", null, "third_party")],
    ["lingxia"],
  );
  assert.equal(result.status, 1);
  assert.match(result.stderr, /missing.*lingxia-rong-command/i);
});

test("rejects packages explicitly excluded from crates.io", () => {
  const result = verify([pkg("lingxia"), pkg("lingxia-cli", [])], ["lingxia", "lingxia-cli"]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /excluded.*lingxia-cli/i);
});

test("permits a registry-restricted crates.io package", () => {
  const result = verify([pkg("lingxia", ["crates-io"])], ["lingxia"]);
  assert.equal(result.status, 0, result.stderr);
});
