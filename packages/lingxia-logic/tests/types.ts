import { definePage, isLxError, lx, type LxError, type TabTarget } from "@lingxia/logic";
import { setLx } from "@lingxia/logic/testing";

// One import line brings the runtime and the page API.
export default definePage({
  actions: {
    async open(target: TabTarget) {
      try {
        await lx.nav.go(target);
      } catch (e) {
        if (isLxError(e) && e.code === "E_NOT_FOUND") return;
        throw e;
      }
    },
  },
});
const failure: LxError | undefined = undefined;
void failure;
setLx(undefined);
import { packageText, type PackageWords } from "@lingxia/logic";
declare const jinshuWords: PackageWords;
const summary: string = packageText(jinshuWords, "@jinshu.agent.edit", { title: "W3" }, "zh-CN");
void summary;
// @ts-expect-error Logic has no page language: pass the caller's.
packageText(jinshuWords, "@jinshu.agent.edit");
