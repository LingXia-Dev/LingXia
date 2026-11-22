//! Every error an `lx` call raises is an `LxError`, refusals included: one `catch`
//! and `instanceof LxError` cover them all.

use rong::{JSContext, JSFunc, JSObject, JSResult, Source};

/// The subclasses keep the built-in `name` an author already logs; they are internal.
const CLASSES: &str = r#"(() => {
  class LxError extends Error {}
  const kind = (name) => {
    const C = class extends LxError {};
    Object.defineProperty(C, 'name', { value: `Lx${name}` });
    Object.defineProperty(C.prototype, 'name', { value: name, writable: true, configurable: true });
    return C;
  };
  Object.defineProperty(globalThis, Symbol.for('lingxia.LxError'), { value: LxError });
  return { Error: LxError, TypeError: kind('TypeError'), RangeError: kind('RangeError'), AbortError: kind('AbortError') };
})()"#;

const NAMES: [&str; 4] = ["Error", "TypeError", "RangeError", "AbortError"];

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let classes: JSObject = ctx.eval(Source::from_bytes(CLASSES))?;
    for name in NAMES {
        let class: JSFunc = classes.get(name)?;
        ctx.register_error_class(name, class)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use rong::{HostError, JSEngine, JSFunc, Source};

    /// `read(undefined)` takes the same strict `String` path as `lx.fs.read(undefined as never)`.
    #[test]
    fn a_refusal_and_a_host_error_are_both_lx_errors() {
        let runtime = <rong::RongJS as JSEngine>::runtime();
        let ctx = runtime.context();
        super::init(&ctx).unwrap();
        let read = JSFunc::new(&ctx, |path: String| path).unwrap();
        ctx.global().set("read", read).unwrap();
        let fail = JSFunc::new(&ctx, |kind: String| -> rong::JSResult<()> {
            let error = HostError::new(rong::error::E_NOT_FOUND, "missing");
            Err(if kind == "abort" {
                HostError::new(rong::error::E_ABORT, "stopped")
                    .with_name("AbortError")
                    .into()
            } else {
                error.into()
            })
        })
        .unwrap();
        ctx.global().set("fail", fail).unwrap();
        let seen: String = ctx
            .eval(Source::from_bytes(
                r#"(() => {
                  const LxError = globalThis[Symbol.for('lingxia.LxError')];
                  const shape = (run) => {
                    try { run(); return 'no throw'; }
                    catch (e) { return [e instanceof LxError, e.code, e.name, typeof e.data?.argument].join(' '); }
                  };
                  return [shape(() => read(undefined)), shape(() => fail('plain')), shape(() => fail('abort'))].join(' | ');
                })()"#,
            ))
            .unwrap();
        assert_eq!(
            seen,
            "true E_INVALID_ARG TypeError number | true E_NOT_FOUND Error undefined | true E_ABORT AbortError undefined"
        );
    }
}
