import assert from "node:assert/strict";
import test from "node:test";
import { checkLayers } from "./check-crate-layers.mjs";

function pkg(name, deps = [], dir = "crates") {
  return {
    name,
    manifest_path: `/repo/${dir}/${name}/Cargo.toml`,
    dependencies: deps.map((dep) => ({ name: dep, kind: null })),
  };
}

test("a crate may depend on its own layer and below", () => {
  const metadata = {
    packages: [
      pkg("lingxia-lxapp", ["lingxia-service", "lingxia-webview", "lingxia-app-context"]),
      pkg("lingxia", ["lingxia-lxapp", "lingxia-native-macros"]),
    ],
  };
  assert.deepEqual(checkLayers(metadata), []);
});

test("a dependency on a higher layer is reported", () => {
  const metadata = { packages: [pkg("lingxia-lxapp", ["lingxia-automation"])] };
  assert.deepEqual(checkLayers(metadata), [
    "lingxia-lxapp (layer 3) depends on lingxia-automation (layer 4), a layer above it",
  ]);
});

test("nothing below the facade may depend on it", () => {
  const metadata = { packages: [pkg("lingxia-automation", ["lingxia"])] };
  assert.equal(checkLayers(metadata).length, 1);
});

test("a crate with no layer is reported, tools are not checked", () => {
  const metadata = {
    packages: [pkg("lingxia-new-thing"), pkg("lingxia-cli", ["lingxia"], "tools")],
  };
  assert.match(checkLayers(metadata)[0], /lingxia-new-thing has no layer/);
  assert.equal(checkLayers(metadata).length, 1);
});

test("dev and build dependencies do not count", () => {
  const lxapp = pkg("lingxia-lxapp");
  lxapp.dependencies.push({ name: "lingxia-automation", kind: "dev" });
  assert.deepEqual(checkLayers({ packages: [lxapp] }), []);
});
