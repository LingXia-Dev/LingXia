import { describe, it, expect } from "vitest";
import { injectPagePath } from "../page-path-injector.js";

const basicSource = `
  Page({
    data: {},
    onLoad() {
      console.log('hello');
    }
  });
`;

describe("injectPagePath", () => {
  it("appends path literal to Page calls lacking second argument", () => {
    const output = injectPagePath(basicSource, "pages/demo/index.js");
    expect(output).toContain(`Page({`);
    expect(output).toContain(`pages/demo/index.js`);
  });

  it("keeps existing second argument untouched", () => {
    const source = `
      Page({ data: {} }, 'pages/kept/index.js');
    `;
    expect(injectPagePath(source, "pages/new/index.js")).toContain(
      `'pages/kept/index.js'`,
    );
  });

  it("handles strings with unmatched braces safely", () => {
    const source = `
      Page({
        onLoad() {
          const tips = 'Press Cmd+} to focus';
          const html = \`
            <view>
              <!-- unmatched } here -->
            </view>
          \`;
        }
      });
    `;
    const output = injectPagePath(source, "pages/braces/index.js");
    expect(output).toContain(`pages/braces/index.js`);
  });

  it("returns original content if no Page call found", () => {
    const source = `console.log('no page here');`;
    expect(injectPagePath(source, "pages/none/index.js")).toBe(source);
  });
});
