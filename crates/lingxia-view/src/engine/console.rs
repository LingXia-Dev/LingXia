//! The View's `console`: lines go to the host's log pipeline; a call never throws.
use super::JSContext;
use rong::Source;
use std::sync::Arc;

/// Where a page's console lines go: the host tags them with the page.
pub type ConsoleSink = Arc<dyn Fn(log::Level, &str) + Send + Sync>;

/// Longer lines are cut, so a logged blob cannot flood the pipeline.
const LINE_BYTES: usize = 8 * 1024;

/// Levels past the logger's maximum return before formatting their arguments.
const SCRIPT: &str = r#"(()=>{const g=globalThis,sink=g.__lingxiaConsole,max=g.__lingxiaConsoleMax;delete g.__lingxiaConsole;delete g.__lingxiaConsoleMax;
const text=v=>{try{switch(typeof v){case'string':return v;case'symbol':return v.toString();case'function':return'[function '+(v.name||'anonymous')+']';case'object':{if(v===null)return'null';if(v instanceof Error)return v.stack?String(v)+'\n'+v.stack:String(v);let s;try{s=JSON.stringify(v)}catch{}return s===undefined?Object.prototype.toString.call(v):s}default:return String(v)}}catch{return'[unprintable]'}};
const method=level=>level>max?()=>{}:(...args)=>{try{sink(level,args.map(text).join(' '))}catch{}};
g.console=Object.freeze({error:method(1),warn:method(2),info:method(3),log:method(3),debug:method(4)});return true})()"#;

/// Installs `console` (`error`, `warn`, `info`, `log`, `debug`); without a sink lines go to the
/// `log` crate.
pub(crate) fn install(context: &JSContext, sink: Option<ConsoleSink>) -> Result<(), String> {
    let sink = rong::quickjs::JSFunc::new(context, move |level: f64, message: String| {
        let level = match level as u8 {
            1 => log::Level::Error,
            2 => log::Level::Warn,
            3 => log::Level::Info,
            _ => log::Level::Debug,
        };
        let message = cut(&message);
        match &sink {
            Some(sink) => sink(level, message),
            None => log::log!(target: "lingxia_view::console", level, "{message}"),
        }
    })
    .map_err(|e| e.to_string())?;
    let global = context.global();
    global
        .set("__lingxiaConsole", sink)
        .map_err(|e| e.to_string())?;
    global
        .set("__lingxiaConsoleMax", log::max_level() as usize as f64)
        .map_err(|e| e.to_string())?;
    context
        .eval::<bool>(Source::from_bytes(SCRIPT))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn cut(message: &str) -> &str {
    if message.len() <= LINE_BYTES {
        return message;
    }
    let mut end = LINE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    &message[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{JSRuntime, RongJS};
    use rong::JSEngine;
    use std::sync::Mutex;

    fn page(lines: &Arc<Mutex<Vec<(log::Level, String)>>>) -> (JSRuntime, JSContext) {
        let runtime = RongJS::runtime();
        let context = runtime.context();
        let lines = lines.clone();
        install(
            &context,
            Some(Arc::new(move |level, message: &str| {
                lines.lock().unwrap().push((level, message.to_owned()))
            })),
        )
        .unwrap();
        (runtime, context)
    }

    #[test]
    fn each_method_logs_its_arguments_joined_at_its_level() {
        log::set_max_level(log::LevelFilter::Trace);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (_runtime, context) = page(&lines);
        context
            .eval::<bool>(Source::from_bytes(
                "console.log('a',1,true,null,undefined,{b:[2]});console.info('i');console.warn('w');console.error(new TypeError('bad'));console.debug(Symbol('s'),function named(){});true",
            ))
            .unwrap();
        let lines = lines.lock().unwrap();
        assert_eq!(
            lines[0],
            (
                log::Level::Info,
                "a 1 true null undefined {\"b\":[2]}".into()
            )
        );
        assert_eq!(lines[1], (log::Level::Info, "i".into()));
        assert_eq!(lines[2], (log::Level::Warn, "w".into()));
        assert_eq!(lines[3].0, log::Level::Error);
        assert!(lines[3].1.starts_with("TypeError: bad"), "{}", lines[3].1);
        assert_eq!(
            lines[4],
            (log::Level::Debug, "Symbol(s) [function named]".into())
        );
    }

    #[test]
    fn values_that_resist_printing_never_throw() {
        log::set_max_level(log::LevelFilter::Trace);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (_runtime, context) = page(&lines);
        context
            .eval::<bool>(Source::from_bytes(
                "const loop={};loop.self=loop;const evil={toJSON(){throw Error('json')}};const hostile=new Proxy({},{get(){throw 1},getPrototypeOf(){throw 1}});console.log(loop,evil,10n,Object.create(null),hostile);console.log('x'.repeat(20000));true",
            ))
            .unwrap();
        let lines = lines.lock().unwrap();
        assert_eq!(
            lines[0].1,
            "[object Object] [object Object] 10 {} [unprintable]"
        );
        assert_eq!(lines[1].1.len(), LINE_BYTES);
    }

    #[test]
    fn console_is_frozen_and_its_sink_is_not_reachable() {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (_runtime, context) = page(&lines);
        let hidden: bool = context
            .eval(Source::from_bytes(
                "Object.isFrozen(console)&&!('__lingxiaConsole' in globalThis)&&!('__lingxiaConsoleMax' in globalThis)",
            ))
            .unwrap();
        assert!(hidden);
    }
}
