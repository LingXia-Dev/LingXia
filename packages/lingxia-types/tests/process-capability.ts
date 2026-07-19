import type {
  RongShellCommand,
  RongSpawnOptions,
  RongSubprocess,
  RongSyncSubprocess,
} from "../src/process.js";

const options: RongSpawnOptions = {
  cwd: "/tmp",
  stdout: "pipe",
  timeout: 5_000,
};

const child: RongSubprocess = Rong.spawn(["echo", "hello"], options);
const syncChild: RongSyncSubprocess = Rong.spawnSync({
  cmd: ["echo", "hello"],
  stderr: "ignore",
});
const shell: RongShellCommand = Rong.$`echo ${"hello"}`.quiet();

void child.stdout?.text();
void syncChild.success;
void shell.text();

// The narrow capability entry must not advertise unrelated Rong modules.
// @ts-expect-error filesystem is not part of capabilities.process
void Rong.file;
