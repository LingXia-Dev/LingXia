// @lingxia/polyfills — ES5 stdlib polyfills shipped as a standalone asset
// when the CLI's legacy HTML pipeline targets ES5 (Android 5.x/6.x stock
// WebView, Chromium 37–44). The CLI's `tsc target=ES5` post-pass downlevels
// SYNTAX; this file's job is to fill the stdlib gaps tsc can't handle.
//
// Each guard is a feature detect — on engines that already have the method,
// the block is a no-op. Don't grow this file casually; every byte ships to
// every Android device.
//
// CRITICAL: this file must be built with terser `compress: false`. Terser's
// aggressive simplification of `typeof X.y !== "function"` patterns against
// known builtin members inverts feature-detect guards — the polyfill ends
// up skipping the engines that need it. See scripts/build.mjs.
(function (root) {
  // No "use strict" — we deliberately mutate globals (Symbol/Set/Map),
  // which strict mode wouldn't allow via bare assignment.

  // Object.assign — Chromium 45+
  if (typeof Object.assign !== "function") {
    Object.defineProperty(Object, "assign", {
      value: function (target) {
        if (target == null) {
          throw new TypeError("Cannot convert undefined or null to object");
        }
        var to = Object(target);
        for (var i = 1; i < arguments.length; i++) {
          var src = arguments[i];
          if (src != null) {
            for (var key in src) {
              if (Object.prototype.hasOwnProperty.call(src, key)) {
                to[key] = src[key];
              }
            }
          }
        }
        return to;
      },
      writable: true,
      configurable: true,
    });
  }

  // Object.entries — Chromium 54+
  if (typeof Object.entries !== "function") {
    Object.entries = function (obj) {
      var keys = Object.keys(obj);
      var out = [];
      for (var i = 0; i < keys.length; i++) {
        out.push([keys[i], obj[keys[i]]]);
      }
      return out;
    };
  }

  // Object.values — Chromium 54+
  if (typeof Object.values !== "function") {
    Object.values = function (obj) {
      var keys = Object.keys(obj);
      var out = [];
      for (var i = 0; i < keys.length; i++) {
        out.push(obj[keys[i]]);
      }
      return out;
    };
  }

  // Array.from — Chromium 45+. Used by the bridge to snapshot Map iterators
  // before mutating them, so this needs to handle Map/Set in addition to
  // array-likes.
  if (typeof Array.from !== "function") {
    Array.from = function (source, mapFn, thisArg) {
      if (source == null) {
        throw new TypeError("Array.from requires an iterable or array-like");
      }
      var out = [];
      // Map/Set iterators expose forEach; use it when length isn't present.
      if (typeof source.length !== "number" && typeof source.forEach === "function") {
        source.forEach(function (v) { out.push(v); });
      } else {
        var len = source.length >>> 0;
        for (var i = 0; i < len; i++) {
          out.push(source[i]);
        }
      }
      if (typeof mapFn === "function") {
        for (var j = 0; j < out.length; j++) {
          out[j] = mapFn.call(thisArg, out[j], j);
        }
      }
      return out;
    };
  }

  // Array.prototype.includes — Chromium 47+
  if (typeof Array.prototype.includes !== "function") {
    Object.defineProperty(Array.prototype, "includes", {
      value: function (target) {
        var len = this.length >>> 0;
        for (var i = 0; i < len; i++) {
          // NaN-safe compare
          if (this[i] === target || (target !== target && this[i] !== this[i])) {
            return true;
          }
        }
        return false;
      },
      writable: true,
      configurable: true,
    });
  }

  // Array.prototype.find / findIndex — Chromium 45+
  if (typeof Array.prototype.find !== "function") {
    Object.defineProperty(Array.prototype, "find", {
      value: function (predicate) {
        var len = this.length >>> 0;
        var thisArg = arguments[1];
        for (var i = 0; i < len; i++) {
          if (predicate.call(thisArg, this[i], i, this)) return this[i];
        }
        return undefined;
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof Array.prototype.findIndex !== "function") {
    Object.defineProperty(Array.prototype, "findIndex", {
      value: function (predicate) {
        var len = this.length >>> 0;
        var thisArg = arguments[1];
        for (var i = 0; i < len; i++) {
          if (predicate.call(thisArg, this[i], i, this)) return i;
        }
        return -1;
      },
      writable: true,
      configurable: true,
    });
  }

  // String.prototype.startsWith / endsWith / includes — Chromium 41+
  if (typeof String.prototype.startsWith !== "function") {
    Object.defineProperty(String.prototype, "startsWith", {
      value: function (search, pos) {
        pos = pos || 0;
        return this.substring(pos, pos + search.length) === String(search);
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof String.prototype.endsWith !== "function") {
    Object.defineProperty(String.prototype, "endsWith", {
      value: function (search, endPos) {
        if (endPos === undefined || endPos > this.length) endPos = this.length;
        return this.substring(endPos - search.length, endPos) === String(search);
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof String.prototype.includes !== "function") {
    Object.defineProperty(String.prototype, "includes", {
      value: function (search, start) {
        return this.indexOf(search, start || 0) !== -1;
      },
      writable: true,
      configurable: true,
    });
  }

  // Number.isFinite — Chromium 19+, but missing on a few stock WebViews.
  if (typeof Number.isFinite !== "function") {
    Number.isFinite = function (value) {
      return typeof value === "number" && isFinite(value);
    };
  }
  if (typeof Number.isInteger !== "function") {
    Number.isInteger = function (value) {
      return typeof value === "number" && isFinite(value) && Math.floor(value) === value;
    };
  }

  // Promise.prototype.finally — Chromium 63+. The page-runtime emits
  // `.then(...).catch(...).finally(...)` chains, which crash with
  // "undefined is not a function" on older WebViews.
  if (typeof Promise !== "undefined" && typeof Promise.prototype.finally !== "function") {
    Object.defineProperty(Promise.prototype, "finally", {
      value: function (callback) {
        var P = this.constructor || Promise;
        return this.then(
          function (value) {
            return P.resolve(callback()).then(function () { return value; });
          },
          function (reason) {
            return P.resolve(callback()).then(function () { throw reason; });
          }
        );
      },
      writable: true,
      configurable: true,
    });
  }
  // Promise.allSettled — Chromium 76+. Cheap to polyfill.
  if (typeof Promise !== "undefined" && typeof Promise.allSettled !== "function") {
    Promise.allSettled = function (promises) {
      return Promise.all(
        Array.prototype.map.call(promises, function (p) {
          return Promise.resolve(p).then(
            function (value) { return { status: "fulfilled", value: value }; },
            function (reason) { return { status: "rejected", reason: reason }; }
          );
        })
      );
    };
  }
  // Promise.any — Chromium 85+. Rejects with AggregateError-ish object if all reject.
  if (typeof Promise !== "undefined" && typeof Promise.any !== "function") {
    Promise.any = function (promises) {
      return new Promise(function (resolve, reject) {
        var arr = [];
        for (var i = 0; i < promises.length; i++) arr.push(promises[i]);
        var errors = new Array(arr.length);
        var remaining = arr.length;
        if (remaining === 0) {
          reject({ name: "AggregateError", errors: [], message: "All promises were rejected" });
          return;
        }
        arr.forEach(function (p, idx) {
          Promise.resolve(p).then(resolve, function (err) {
            errors[idx] = err;
            if (--remaining === 0) {
              reject({ name: "AggregateError", errors: errors, message: "All promises were rejected" });
            }
          });
        });
      });
    };
  }

  // Object.fromEntries — Chromium 73+. Used by lingxia-react components.
  if (typeof Object.fromEntries !== "function") {
    Object.fromEntries = function (iterable) {
      var out = {};
      if (iterable == null) return out;
      if (typeof iterable.forEach === "function") {
        iterable.forEach(function (pair) {
          if (pair) out[pair[0]] = pair[1];
        });
      } else {
        for (var i = 0; i < iterable.length; i++) {
          var pair = iterable[i];
          if (pair) out[pair[0]] = pair[1];
        }
      }
      return out;
    };
  }

  // String.prototype.padStart / padEnd — Chromium 57+. Used by lingxia-elements/picker.
  if (typeof String.prototype.padStart !== "function") {
    Object.defineProperty(String.prototype, "padStart", {
      value: function (targetLength, padString) {
        var s = String(this);
        if (s.length >= targetLength) return s;
        var pad = String(padString === undefined ? " " : padString);
        if (pad.length === 0) return s;
        var need = targetLength - s.length;
        var prefix = "";
        while (prefix.length < need) prefix += pad;
        return prefix.slice(0, need) + s;
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof String.prototype.padEnd !== "function") {
    Object.defineProperty(String.prototype, "padEnd", {
      value: function (targetLength, padString) {
        var s = String(this);
        if (s.length >= targetLength) return s;
        var pad = String(padString === undefined ? " " : padString);
        if (pad.length === 0) return s;
        var need = targetLength - s.length;
        var suffix = "";
        while (suffix.length < need) suffix += pad;
        return s + suffix.slice(0, need);
      },
      writable: true,
      configurable: true,
    });
  }
  // String.prototype.trimStart / trimEnd — Chromium 66+.
  if (typeof String.prototype.trimStart !== "function") {
    Object.defineProperty(String.prototype, "trimStart", {
      value: function () {
        return String(this).replace(/^[\s﻿\xA0]+/, "");
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof String.prototype.trimEnd !== "function") {
    Object.defineProperty(String.prototype, "trimEnd", {
      value: function () {
        return String(this).replace(/[\s﻿\xA0]+$/, "");
      },
      writable: true,
      configurable: true,
    });
  }
  // String.prototype.replaceAll — Chromium 85+.
  if (typeof String.prototype.replaceAll !== "function") {
    Object.defineProperty(String.prototype, "replaceAll", {
      value: function (search, replacement) {
        if (search instanceof RegExp) {
          if (!search.global) {
            throw new TypeError("String.prototype.replaceAll called with a non-global RegExp argument");
          }
          return String(this).replace(search, replacement);
        }
        // Build a global regex from the escaped literal.
        var str = String(search).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        return String(this).replace(new RegExp(str, "g"), replacement);
      },
      writable: true,
      configurable: true,
    });
  }

  // Array.prototype.flat / flatMap — Chromium 69+.
  if (typeof Array.prototype.flat !== "function") {
    Object.defineProperty(Array.prototype, "flat", {
      value: function (depth) {
        var d = depth === undefined ? 1 : Number(depth);
        var out = [];
        (function rec(arr, level) {
          for (var i = 0; i < arr.length; i++) {
            var v = arr[i];
            if (Array.isArray(v) && level > 0) rec(v, level - 1);
            else out.push(v);
          }
        })(this, d);
        return out;
      },
      writable: true,
      configurable: true,
    });
  }
  if (typeof Array.prototype.flatMap !== "function") {
    Object.defineProperty(Array.prototype, "flatMap", {
      value: function (callback, thisArg) {
        var mapped = Array.prototype.map.call(this, callback, thisArg);
        return Array.prototype.flat.call(mapped, 1);
      },
      writable: true,
      configurable: true,
    });
  }
  // Array.prototype.at — Chromium 92+. Sugar for negative indexing.
  if (typeof Array.prototype.at !== "function") {
    Object.defineProperty(Array.prototype, "at", {
      value: function (index) {
        var n = Math.trunc(Number(index)) || 0;
        if (n < 0) n += this.length;
        if (n < 0 || n >= this.length) return undefined;
        return this[n];
      },
      writable: true,
      configurable: true,
    });
  }

  // globalThis — Chromium 71+. Can't polyfill the keyword itself, but we can
  // assign a property on the global object so `typeof globalThis !== "undefined"`
  // checks pass. Code that uses `globalThis.X` will still need the property
  // to exist on `window` already (which is the same object).
  if (typeof root.globalThis === "undefined") {
    try { root.globalThis = root; } catch (e) { /* property may be non-writable */ }
  }

  // Android 5.0 stock WebView (Chromium 37) is missing Symbol, Set, Map.
  // 5.1 (Chromium 39) has them. Below shims are intentionally minimal —
  // they cover the bridge/page-runtime usage and nothing else.

  // Symbol — Chromium 38+. Real Symbol can't be polyfilled; this shim
  // returns unique strings, so `typeof Symbol(...) === "symbol"` will be
  // false. Callers in our codebase only use Symbol as a unique key for
  // Object.defineProperty / property access, which works fine with strings.
  var symbolCounter = 0;
  if (typeof Symbol === "undefined") {
    var SymbolShim = function (description) {
      var key = "@@_sym_" + (++symbolCounter) + "_" + (description || "");
      return key;
    };
    SymbolShim.iterator = SymbolShim("Symbol.iterator");
    SymbolShim.asyncIterator = SymbolShim("Symbol.asyncIterator");
    SymbolShim.toStringTag = SymbolShim("Symbol.toStringTag");
    var symbolForRegistry = {};
    SymbolShim["for"] = function (key) {
      var k = String(key);
      if (!Object.prototype.hasOwnProperty.call(symbolForRegistry, k)) {
        symbolForRegistry[k] = SymbolShim(k);
      }
      return symbolForRegistry[k];
    };
    root.Symbol = SymbolShim;
  }

  // Set — Chromium 38+. Minimal: add/has/delete/clear/forEach/size.
  if (typeof Set === "undefined") {
    var SetShim = function (iterable) {
      this._items = [];
      var self = this;
      if (iterable != null) {
        if (typeof iterable.forEach === "function") {
          iterable.forEach(function (v) { self.add(v); });
        } else if (typeof iterable.length === "number") {
          for (var i = 0; i < iterable.length; i++) self.add(iterable[i]);
        }
      }
    };
    SetShim.prototype.add = function (value) {
      if (this._items.indexOf(value) === -1) this._items.push(value);
      return this;
    };
    SetShim.prototype.has = function (value) {
      return this._items.indexOf(value) !== -1;
    };
    SetShim.prototype["delete"] = function (value) {
      var idx = this._items.indexOf(value);
      if (idx === -1) return false;
      this._items.splice(idx, 1);
      return true;
    };
    SetShim.prototype.clear = function () { this._items.length = 0; };
    SetShim.prototype.forEach = function (callback, thisArg) {
      // Snapshot to mirror native semantics: re-entrant add during iteration
      // doesn't affect this pass.
      var copy = this._items.slice();
      for (var i = 0; i < copy.length; i++) {
        callback.call(thisArg, copy[i], copy[i], this);
      }
    };
    Object.defineProperty(SetShim.prototype, "size", {
      get: function () { return this._items.length; },
    });
    root.Set = SetShim;
  }

  // Map — Chromium 38+. Minimal: get/set/has/delete/clear/forEach/size/keys/values/entries.
  if (typeof Map === "undefined") {
    var MapShim = function (iterable) {
      this._keys = [];
      this._values = [];
      var self = this;
      if (iterable != null && typeof iterable.forEach === "function") {
        iterable.forEach(function (entry) { self.set(entry[0], entry[1]); });
      } else if (iterable != null && typeof iterable.length === "number") {
        for (var i = 0; i < iterable.length; i++) {
          self.set(iterable[i][0], iterable[i][1]);
        }
      }
    };
    MapShim.prototype.get = function (key) {
      var idx = this._keys.indexOf(key);
      return idx === -1 ? undefined : this._values[idx];
    };
    MapShim.prototype.set = function (key, value) {
      var idx = this._keys.indexOf(key);
      if (idx === -1) {
        this._keys.push(key);
        this._values.push(value);
      } else {
        this._values[idx] = value;
      }
      return this;
    };
    MapShim.prototype.has = function (key) {
      return this._keys.indexOf(key) !== -1;
    };
    MapShim.prototype["delete"] = function (key) {
      var idx = this._keys.indexOf(key);
      if (idx === -1) return false;
      this._keys.splice(idx, 1);
      this._values.splice(idx, 1);
      return true;
    };
    MapShim.prototype.clear = function () {
      this._keys.length = 0;
      this._values.length = 0;
    };
    MapShim.prototype.forEach = function (callback, thisArg) {
      var keysCopy = this._keys.slice();
      var valuesCopy = this._values.slice();
      for (var i = 0; i < keysCopy.length; i++) {
        callback.call(thisArg, valuesCopy[i], keysCopy[i], this);
      }
    };
    MapShim.prototype.keys = function () { return this._keys.slice(); };
    MapShim.prototype.values = function () { return this._values.slice(); };
    MapShim.prototype.entries = function () {
      var out = [];
      for (var i = 0; i < this._keys.length; i++) {
        out.push([this._keys[i], this._values[i]]);
      }
      return out;
    };
    Object.defineProperty(MapShim.prototype, "size", {
      get: function () { return this._keys.length; },
    });
    root.Map = MapShim;
  }
})(typeof window !== "undefined" ? window : this);
