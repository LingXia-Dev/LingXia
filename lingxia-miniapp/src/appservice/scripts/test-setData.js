// Run using: node test-setData.js

const fs = require("fs");
const path = require("path");
const vm = require("vm");

// Mock _Page
let mockPageStore = {};
function _Page(pageConfig) {
  const mockInstance = {
    // Test Hooks
    _test_lastJsonSent: null, // Stores the JSON string passed to the last mock _setData call
    _test_lastCallbackSent: null, // Stores the callback passed to the last mock _setData call

    // Mocked Native Methods
    _setData: function (jsonData, callback) {
      console.log(
        `[Mock _setData] Received JSON: ${jsonData.substring(0, 100)}${jsonData.length > 100 ? "..." : ""}`,
      ); // Log received data concisely
      this._test_lastJsonSent = jsonData;
      this._test_lastCallbackSent = callback;
      setTimeout(() => {
        if (typeof callback === "function") {
          try {
            callback();
          } catch (e) {
            console.error("[Lingxia Mock] Error in mock _setData callback:", e);
          }
        }
      }, 50);
    },
  };
  mockPageStore[pageConfig.route || `mockPage_${Date.now()}`] = mockInstance;
  return mockInstance;
}

let testsPassed = 0;
let testsFailed = 0;
const testResults = [];

function assert(condition, message) {
  if (!condition) {
    throw new Error(`Assertion Failed: ${message}`);
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function loadSetDataScriptOnce() {
  try {
    global._Page = _Page; // Inject mock
    global.Page = undefined; // Ensure wrapper overwrites
    const scriptPath = path.join(__dirname, "Page.js");
    const scriptCode = fs.readFileSync(scriptPath, "utf8");
    vm.runInThisContext(scriptCode, { filename: "Page.js" });
    assert(
      typeof Page === "function",
      "Wrapped Page function should be globally defined.",
    );
    console.log("--- Page.js loaded (using mock _Page) ---");
  } catch (e) {
    console.error("FATAL: Failed to load Page.js:", e);
    process.exit(1);
  }
}

async function testBasicSetData() {
  const testName = "Basic setData";
  const pageConfig = { data: { count: 0 } };
  const pageInstance = Page(pageConfig);
  assert(pageInstance !== null, `${testName}: Instance creation.`);
  assert(pageInstance.data.count === 0, `${testName}: Initial data.`);
  let callbackCalled = false;
  pageInstance.setData({ count: 1 }, () => {
    callbackCalled = true;
  });
  assert(pageInstance.data.count === 1, `${testName}: Sync update.`);
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  const expectedPatch = { count: 1 };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
  assert(callbackCalled === true, `${testName}: Callback.`);
}

async function testDebounce() {
  const testName = "Debounce";
  const pageConfig = { data: { a: 1, b: 1 } };
  const pageInstance = Page(pageConfig);
  let callback1Called = false;
  let callback2Called = false;
  let _setDataCallCount = 0;
  const original_setData = pageInstance._setData;
  pageInstance._setData = function (jsonData, callback) {
    _setDataCallCount++;
    original_setData.call(this, jsonData, callback);
  };
  pageInstance.setData({ a: 2 }, () => {
    callback1Called = true;
  });
  assert(pageInstance.data.a === 2, `${testName}: Sync update 1.`);
  console.log(
    `[Test Log] ${testName}: Data after setData 1:`,
    JSON.stringify(pageInstance.data),
  );
  pageInstance.setData({ b: 3 }, () => {
    callback2Called = true;
  });
  assert(pageInstance.data.b === 3, `${testName}: Sync update 2.`);
  console.log(
    `[Test Log] ${testName}: Data after setData 2:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  assert(_setDataCallCount === 1, `${testName}: Call count.`);
  const expectedPatch = { a: 2, b: 3 };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
  assert(callback1Called === true, `${testName}: Callback 1.`);
  assert(callback2Called === true, `${testName}: Callback 2.`);
}

async function testPathSettingObject() {
  const testName = "Path Object";
  const pageConfig = {
    data: { user: { name: "A", address: { city: "X" } }, other: 1 },
  };
  const pageInstance = Page(pageConfig);
  pageInstance.setData({ "user.name": "B", "user.address.city": "Y" });
  assert(pageInstance.data.user.name === "B", `${testName}: Sync name.`);
  assert(
    pageInstance.data.user.address.city === "Y",
    `${testName}: Sync city.`,
  );
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  const expectedPatch = { "user.name": "B", "user.address.city": "Y" };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
}

async function testPathSettingArray() {
  const testName = "Path Array Index";
  const pageConfig = { data: { list: [1, 2, 3], other: "z" } };
  const pageInstance = Page(pageConfig);
  pageInstance.setData({ "list[1]": 99 });
  assert(Array.isArray(pageInstance.data.list), `${testName}: Is array.`);
  assert(pageInstance.data.list.length === 3, `${testName}: Length.`);
  assert(pageInstance.data.list[1] === 99, `${testName}: Sync update.`);
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  const expectedPatch = { list: [1, 99, 3] };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
}

async function testPathSettingMixed() {
  const testName = "Path Mixed Object/Array";
  const pageConfig = {
    data: {
      config: { enabled: true },
      items: [
        { id: 1, val: "a" },
        { id: 2, val: "b" },
      ],
    },
  };
  const pageInstance = Page(pageConfig);
  pageInstance.setData({ "items[0].val": "A", "items[1].id": 22 });
  assert(
    pageInstance.data.items[0].val === "A",
    `${testName}: Sync items[0].val.`,
  );
  assert(
    pageInstance.data.items[1].id === 22,
    `${testName}: Sync items[1].id.`,
  );
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  const expectedPatch = {
    items: [
      { id: 1, val: "A" },
      { id: 22, val: "b" },
    ],
  };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
}

async function testPathCreation() {
  const testName = "Path Creation";
  const pageConfig = { data: {} };
  const pageInstance = Page(pageConfig);
  pageInstance.setData({ "a.b.c": 1, "list[0]": "hello", "list[2]": "world" });
  assert(
    typeof pageInstance.data.a?.b?.c === "number",
    `${testName}: Sync a.b.c.`,
  );
  assert(Array.isArray(pageInstance.data.list), `${testName}: Array created.`);
  // Note: list will be ["hello", empty, "world"]
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);
  const expectedPatch = { a: { b: { c: 1 } }, list: ["hello", null, "world"] };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
}

async function testNoChange() {
  const testName = "No Change";
  const pageConfig = { data: { val: 10 } };
  const pageInstance = Page(pageConfig);
  let callbackCalled = false;
  let setDataCalled = false;
  pageInstance._setData = function () {
    setDataCalled = true;
  }; // Override to detect call

  pageInstance.setData({ val: 10 }, () => {
    callbackCalled = true;
  }); // Set same value

  assert(pageInstance.data.val === 10, `${testName}: Value unchanged.`);
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  );
  await sleep(100);

  assert(
    setDataCalled === false,
    `${testName}: _setData should not be called.`,
  );
  assert(callbackCalled === true, `${testName}: Callback should still run.`);
}

async function testDeleteKey() {
  const testName = "Delete Key";
  const pageConfig = { data: { a: 1, b: 2 } };
  const pageInstance = Page(pageConfig);
  pageInstance.setData({ b: undefined }); // Setting to undefined should delete the key

  assert(
    pageInstance.data.b === undefined,
    `${testName}: Sync delete check value.`,
  ); // Value becomes undefined
  assert(
    !Object.prototype.hasOwnProperty.call(pageInstance.data, "b"),
    `${testName}: Sync delete check hasOwnProperty.`,
  ); // Key should be gone
  assert(pageInstance.data.a === 1, `${testName}: Other key affected.`);
  console.log(
    `[Test Log] ${testName}: Data after setData:`,
    JSON.stringify(pageInstance.data),
  ); // (key 'b' should be gone)
  await sleep(100);

  const expectedPatch = { b: undefined };
  assert(
    pageInstance._test_lastJsonSent === JSON.stringify(expectedPatch),
    `${testName}: Sent patch.`,
  );
}

async function runTests() {
  loadSetDataScriptOnce();

  const tests = [
    testBasicSetData,
    testDebounce,
    testPathSettingObject,
    testPathSettingArray,
    testPathSettingMixed,
    testPathCreation,
    testNoChange,
    testDeleteKey,
  ];

  for (const testFn of tests) {
    const testName = testFn.name;
    console.log(`\n--- Running Test: ${testName} ---`);
    try {
      await testFn();
      testResults.push({ name: testName, status: "✅ PASSED" });
      testsPassed++;
      console.log(`--- Finished Test: ${testName} (PASSED) ---`);
    } catch (error) {
      testResults.push({
        name: testName,
        status: `❌ FAILED`,
        error: error.message,
      });
      testsFailed++;
      console.error(`\nError in test: ${testName}`);
      console.error(error);
      console.log(`--- Finished Test: ${testName} (FAILED) ---`);
    }
  }

  console.log("\n--- Test Results Summary ---");
  testResults.forEach((result) => {
    console.log(
      `${result.status} - ${result.name}${result.error ? ` (${result.error})` : ""}`,
    );
  });
  console.log("---------------------------");
  console.log(`Total: ${testsPassed} passed, ${testsFailed} failed.`);
  console.log("---------------------------");
  process.exit(testsFailed > 0 ? 1 : 0);
}

runTests();
