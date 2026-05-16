//! TypeScript code generation for `#[lingxia::native]` host handlers.
//!
//! Scans Rust source files for `#[lingxia::native("route")]` / `#[native("route")]`
//! function attributes and `pub struct` definitions, then generates a `.ts` module
//! with typed `invoke` / `stream` / `channel` bindings.
//!
//! Intended as a build-dependency so `build.rs` can produce the types during
//! `cargo build`, before the lxapp is assembled.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use syn::{Attribute, FnArg, ItemFn, ItemStruct, ReturnType};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Scan `rust_dir` (recursively) for `#[lingxia::native]` / `#[native]` handlers
/// and struct definitions, then write a native client to `out_path`.
///
/// If no native handlers are found the output file is removed (clean slate).
pub fn generate(rust_dir: &Path, out: &Path) -> Result<()> {
    generate_native_client_from_paths(rust_dir, out)
}

/// Compatibility entry point used by native build scripts.
pub fn generate_native_client_from_paths(rust_dir: &Path, out: &Path) -> Result<()> {
    if !rust_dir.exists() {
        return Err(anyhow!(
            "Native Rust API directory not found: {}",
            rust_dir.display()
        ));
    }
    let manifest = scan(rust_dir)?;
    if manifest.routes.is_empty() {
        let _ = fs::remove_file(out);
        return Ok(());
    }
    let generated = render(&manifest, output_kind(out))?;
    let needs_write = fs::read_to_string(out)
        .map(|existing| existing != generated)
        .unwrap_or(true);
    if needs_write {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(out, generated).with_context(|| format!("Failed to write {}", out.display()))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    TypeScriptModule,
    BrowserGlobalJs,
}

fn output_kind(out: &Path) -> OutputKind {
    match out.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("js") => OutputKind::BrowserGlobalJs,
        _ => OutputKind::TypeScriptModule,
    }
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteKind {
    Call,
    Stream,
    Channel,
}

#[derive(Debug, Clone)]
struct NativeRoute {
    route: String,
    kind: RouteKind,
    input: Option<String>,
    output: Option<String>,
    event: Option<String>,
    channel_in: Option<String>,
    channel_out: Option<String>,
}

#[derive(Debug, Clone)]
struct StructField {
    name: String,
    ty: String,
    optional: bool,
}

#[derive(Debug)]
struct NativeManifest {
    routes: Vec<NativeRoute>,
    structs: BTreeMap<String, Vec<StructField>>,
}

// ---------------------------------------------------------------------------
// Scanner (syn-based)
// ---------------------------------------------------------------------------

fn scan(src_dir: &Path) -> Result<NativeManifest> {
    let mut manifest = NativeManifest {
        routes: Vec::new(),
        structs: BTreeMap::new(),
    };

    let mut files = Vec::new();
    collect_rs_files(src_dir, &mut files).map_err(|e| anyhow!("scan: {e}"))?;

    for file in &files {
        let source =
            fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let ast = syn::parse_file(&source).with_context(|| format!("parse {}", file.display()))?;

        for item in &ast.items {
            if let syn::Item::Fn(item_fn) = item {
                if let Some((route, kind)) = parse_attr(&item_fn.attrs) {
                    manifest
                        .routes
                        .push(extract_route_info(&route, kind, item_fn));
                }
            }
            if let syn::Item::Struct(item_struct) = item {
                let fields = extract_struct_fields(item_struct);
                if !fields.is_empty() {
                    manifest
                        .structs
                        .insert(item_struct.ident.to_string(), fields);
                }
            }
        }
    }

    // Collision check.
    let mut seen = BTreeSet::new();
    for r in &manifest.routes {
        if !seen.insert(r.route.clone()) {
            return Err(anyhow!("duplicate native route `{}`", r.route));
        }
    }

    manifest.routes.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(manifest)
}

fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Match `#[lingxia::native("route")]` or `#[native("route")]` with optional flags.
fn parse_attr(attrs: &[Attribute]) -> Option<(String, RouteKind)> {
    for attr in attrs {
        let is_match = attr.path().is_ident("native")
            || attr
                .path()
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::")
                .ends_with("lingxia::native");
        if !is_match {
            continue;
        }

        let args: String = attr
            .meta
            .require_list()
            .ok()?
            .tokens
            .clone()
            .into_iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join("");

        let route = args.split('"').nth(1).map(str::to_owned)?;
        let rest = args.split('"').nth(2).unwrap_or("");

        let kind = if rest.contains("channel") {
            RouteKind::Channel
        } else if rest.contains("stream") {
            RouteKind::Stream
        } else {
            RouteKind::Call
        };

        return Some((route, kind));
    }
    None
}

fn extract_route_info(route: &str, kind: RouteKind, item_fn: &ItemFn) -> NativeRoute {
    let mut input: Option<String> = None;
    let mut event: Option<String> = None;
    let mut channel_in: Option<String> = None;
    let mut channel_out: Option<String> = None;
    let mut output: Option<String> = None;

    for arg in &item_fn.sig.inputs {
        let FnArg::Typed(pat_type) = arg else {
            continue;
        };
        let ty_str = type_string(&pat_type.ty).replace(' ', "");

        if ty_str.contains("LxApp") || ty_str.contains("HostCancel") {
            continue;
        }

        if ty_str.contains("StreamContext") {
            let args = extract_generic_args(&ty_str, "StreamContext");
            event = args.first().cloned();
            if let Some(result) = args.get(1) {
                output = Some(result.clone());
            }
            continue;
        }

        if ty_str.contains("ChannelContext") {
            let args = extract_generic_args(&ty_str, "ChannelContext");
            channel_in = args.first().cloned();
            channel_out = args.get(1).cloned().or_else(|| channel_in.clone());
            continue;
        }

        input = Some(ty_str);
    }

    if output.is_none() {
        output = match &item_fn.sig.output {
            ReturnType::Type(_, ty) => {
                let s = type_string(ty).replace(' ', "");
                unwrap_result(&s)
            }
            ReturnType::Default => Some("void".to_string()),
        };
    }

    NativeRoute {
        route: route.to_string(),
        kind,
        input,
        output,
        event,
        channel_in,
        channel_out,
    }
}

fn extract_struct_fields(item: &ItemStruct) -> Vec<StructField> {
    item.fields
        .iter()
        .filter_map(|field| {
            let name = field.ident.as_ref()?.to_string();
            let ty_str = type_string(&field.ty).replace(' ', "");
            let optional = ty_str.starts_with("Option<");
            Some(StructField {
                name: to_camel_case(&name),
                ty: ty_str,
                optional,
            })
        })
        .collect()
}

fn extract_generic_args(ty: &str, wrapper: &str) -> Vec<String> {
    let Some(pos) = ty
        .rfind(&format!("{wrapper}<"))
        .or_else(|| ty.find(&format!("{wrapper}<")))
    else {
        return vec![];
    };
    let start = pos + wrapper.len();
    if ty.as_bytes().get(start) != Some(&b'<') {
        return vec![];
    }
    let Some(end) = matching_angle(ty, start) else {
        return vec![];
    };
    let body = match ty.get(start + 1..end) {
        Some(body) => body,
        None => return vec![],
    };
    split_args(body)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].to_string());
    out
}

fn unwrap_result(ty: &str) -> Option<String> {
    for wrapper in &[
        "Result",
        "std::result::Result",
        "HostResult",
        "lingxia::Result",
    ] {
        let args = extract_generic_args(ty, wrapper);
        if !args.is_empty() {
            let inner = args.first().cloned().unwrap_or_else(|| "void".to_string());
            return Some(inner.trim().to_string());
        }
    }
    if ty == "()" {
        Some("void".to_string())
    } else {
        Some(ty.to_string())
    }
}

// ---------------------------------------------------------------------------
// TypeScript rendering
// ---------------------------------------------------------------------------

fn render(manifest: &NativeManifest, output_kind: OutputKind) -> Result<String> {
    match output_kind {
        OutputKind::TypeScriptModule => render_ts_module(manifest),
        OutputKind::BrowserGlobalJs => render_browser_global_js(manifest),
    }
}

fn render_ts_module(manifest: &NativeManifest) -> Result<String> {
    let mut used_types = BTreeSet::new();
    for r in &manifest.routes {
        collect_type_ref(r.input.as_deref(), &mut used_types);
        collect_type_ref(r.output.as_deref(), &mut used_types);
        collect_type_ref(r.event.as_deref(), &mut used_types);
        collect_type_ref(r.channel_in.as_deref(), &mut used_types);
        collect_type_ref(r.channel_out.as_deref(), &mut used_types);
    }

    let mut out = String::new();
    out.push_str("// Generated by `cargo build`. Do not edit by hand.\n");
    out.push_str("import { channel, invoke, stream } from \"@lingxia/bridge\";\n");
    out.push_str("import type { NativeChannel, NativeStream } from \"@lingxia/bridge\";\n\n");
    out.push_str("export type NativeVoid = void;\n\n");

    for ty in &used_types {
        if !is_builtin_ts(ty) {
            if let Some(fields) = manifest.structs.get(ty) {
                out.push_str(&format!("export interface {ty} {{\n"));
                for f in fields {
                    let opt = if f.optional { "?" } else { "" };
                    out.push_str(&format!(
                        "  {}{}: {};\n",
                        f.name,
                        opt,
                        rust_to_ts(clean_option(&f.ty))
                    ));
                }
                out.push_str("}\n\n");
            } else {
                out.push_str(&format!("export type {ty} = unknown;\n\n"));
            }
        }
    }

    let tree = RouteNode::build(&manifest.routes)?;
    out.push_str("export const native = ");
    out.push_str(&tree.render(0));
    out.push_str(";\n");
    Ok(out)
}

fn render_browser_global_js(manifest: &NativeManifest) -> Result<String> {
    let tree = RouteNode::build(&manifest.routes)?;
    let mut out = String::new();
    out.push_str(NATIVE_CLIENT_JS_PREAMBLE);
    out.push_str("  global.native = ");
    out.push_str(&tree.render_js(2));
    out.push_str(NATIVE_CLIENT_JS_FOOTER);
    Ok(out)
}

const NATIVE_CLIENT_JS_PREAMBLE: &str = r#"// Generated by `cargo build`. Do not edit by hand.
(function (global) {
  function bridge() {
    if (!global.LingXiaBridge) throw new Error('window.LingXiaBridge is not available');
    return global.LingXiaBridge;
  }
  function route(parts) {
    return 'host.' + parts.join('.');
  }
  function nativeError(error) {
    if (error && typeof error === 'object') {
      var code = typeof error.code === 'string' && error.code ? error.code : 'BRIDGE_INTERNAL_ERROR';
      var message = typeof error.message === 'string' && error.message ? error.message : 'Unknown error';
      var out = { code: code, message: message };
      if ('data' in error) out.data = error.data;
      return out;
    }
    return { code: 'BRIDGE_INTERNAL_ERROR', message: error instanceof Error ? error.message : String(error || 'Unknown error') };
  }
  function call(parts) {
    return function (input) {
      return bridge().raw.call(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).catch(function (error) { return Promise.reject(nativeError(error)); });
    };
  }
  function stream(parts) {
    return function (input) {
      var handle = bridge().raw.stream(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host', timeoutMs: 0 });
      var eventListeners = [];
      var errorListeners = [];
      handle.on('data', function (event) { eventListeners.slice().forEach(function (listener) { listener(event); }); });
      handle.on('error', function (error) { var normalized = nativeError(error); errorListeners.slice().forEach(function (listener) { listener(normalized); }); });
      return {
        onEvent: function (listener) { eventListeners.push(listener); return function () { eventListeners = eventListeners.filter(function (item) { return item !== listener; }); }; },
        onError: function (listener) { errorListeners.push(listener); return function () { errorListeners = errorListeners.filter(function (item) { return item !== listener; }); }; },
        result: handle.result.catch(function (error) { return Promise.reject(nativeError(error)); }),
        cancel: function () { handle.cancel(); }
      };
    };
  }
  function channel(parts) {
    return function (input) {
      return bridge().raw.channel.open(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).then(function (handle) {
        var messageListeners = [];
        var closeListeners = [];
        handle.on('data', function (message) { messageListeners.slice().forEach(function (listener) { listener(message); }); });
        handle.on('close', function (code, reason) { var event = { code: code, reason: reason }; closeListeners.slice().forEach(function (listener) { listener(event); }); });
        return {
          send: function (message) { handle.send(message); },
          onMessage: function (listener) { messageListeners.push(listener); return function () { messageListeners = messageListeners.filter(function (item) { return item !== listener; }); }; },
          onClose: function (listener) { closeListeners.push(listener); return function () { closeListeners = closeListeners.filter(function (item) { return item !== listener; }); }; },
          close: function (code, reason) { handle.close(code, reason); }
        };
      }).catch(function (error) { return Promise.reject(nativeError(error)); });
    };
  }
"#;

const NATIVE_CLIENT_JS_FOOTER: &str = r#";
})(window);
"#;

fn collect_type_ref(ty: Option<&str>, set: &mut BTreeSet<String>) {
    let Some(ty) = ty else { return };
    let cleaned = ty.trim();
    if cleaned.is_empty() || cleaned == "void" || cleaned == "()" {
        return;
    }
    for wrapper in &["Option", "Vec"] {
        let args = extract_generic_args(cleaned, wrapper);
        if !args.is_empty() {
            collect_type_ref(args.first().map(String::as_str), set);
            return;
        }
    }
    for wrapper in &["HashMap", "BTreeMap"] {
        let args = extract_generic_args(cleaned, wrapper);
        if !args.is_empty() {
            collect_type_ref(args.get(1).map(String::as_str), set);
            return;
        }
    }
    let base = type_basename(cleaned);
    if !is_builtin_ts(base) && base.chars().next().is_some_and(|ch| ch.is_uppercase()) {
        set.insert(base.to_string());
    }
}

fn is_builtin_ts(ty: &str) -> bool {
    matches!(
        ty,
        "string"
            | "boolean"
            | "number"
            | "void"
            | "()"
            | "unknown"
            | "any"
            | "never"
            | "String"
            | "bool"
    )
}

fn rust_to_ts(ty: &str) -> String {
    let ty = ty.trim().trim_start_matches('&').trim_start_matches("mut ");
    if let Some(inner) = clean_option(ty)
        .strip_prefix("Vec<")
        .and_then(|r| r.strip_suffix('>'))
    {
        return format!("{}[]", rust_to_ts(inner));
    }
    let option_args = extract_generic_args(ty, "Option");
    if let Some(inner) = option_args.first() {
        return rust_to_ts(inner);
    }
    let vec_args = extract_generic_args(ty, "Vec");
    if let Some(inner) = vec_args.first() {
        return format!("{}[]", rust_to_ts(inner));
    }
    for wrapper in ["HashMap", "BTreeMap"] {
        let args = extract_generic_args(ty, wrapper);
        if let Some(value) = args.get(1) {
            return format!("Record<string, {}>", rust_to_ts(value));
        }
    }
    match type_basename(ty) {
        "String" | "str" => "string".to_string(),
        "bool" => "boolean".to_string(),
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" | "f32"
        | "f64" => "number".to_string(),
        "()" | "void" => "void".to_string(),
        "Value" | "JsonValue" => "unknown".to_string(),
        other => other.to_string(),
    }
}

fn clean_option(ty: &str) -> &str {
    ty.strip_prefix("Option<")
        .and_then(|r| r.strip_suffix('>'))
        .unwrap_or(ty)
}

fn type_basename(ty: &str) -> &str {
    ty.trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .split("::")
        .last()
        .unwrap_or(ty)
}

// ---------------------------------------------------------------------------
// Route tree → nested TypeScript object literal
// ---------------------------------------------------------------------------

#[derive(Default)]
struct RouteNode {
    children: BTreeMap<String, RouteNode>,
    route: Option<NativeRoute>,
}

impl RouteNode {
    fn build(routes: &[NativeRoute]) -> Result<Self> {
        let mut root = RouteNode::default();
        for r in routes {
            let mut node = &mut root;
            for part in r.route.split('.') {
                if part.trim().is_empty() {
                    return Err(anyhow!("invalid native route `{}`", r.route));
                }
                if node.route.is_some() {
                    return Err(anyhow!(
                        "native route `{}` conflicts with route prefix",
                        r.route
                    ));
                }
                node = node.children.entry(part.to_string()).or_default();
            }
            if node.route.is_some() || !node.children.is_empty() {
                return Err(anyhow!(
                    "native route `{}` conflicts with existing route namespace",
                    r.route
                ));
            }
            node.route = Some(r.clone());
        }
        Ok(root)
    }

    fn render(&self, indent: usize) -> String {
        if let Some(route) = &self.route {
            return render_route_method(route);
        }
        let pad = " ".repeat(indent);
        let child_pad = " ".repeat(indent + 2);
        let mut out = String::from("{\n");
        for (name, child) in &self.children {
            out.push_str(&format!(
                "{child_pad}{}: {},\n",
                safe_ts_property(name),
                child.render(indent + 2)
            ));
        }
        out.push_str(&format!("{pad}}}"));
        out
    }

    fn render_js(&self, indent: usize) -> String {
        if let Some(route) = &self.route {
            return render_js_route_method(route);
        }
        let pad = " ".repeat(indent);
        let child_pad = " ".repeat(indent + 2);
        let mut out = String::from("{\n");
        for (name, child) in &self.children {
            out.push_str(&format!(
                "{child_pad}{}: {},\n",
                safe_ts_property(name),
                child.render_js(indent + 2)
            ));
        }
        out.push_str(&format!("{pad}}}"));
        out
    }
}

fn render_route_method(route: &NativeRoute) -> String {
    let input_ts = route.input.as_deref().map(rust_to_ts);
    let input_arg = input_ts
        .as_ref()
        .map(|ty| format!("input: {ty}"))
        .unwrap_or_default();

    match route.kind {
        RouteKind::Call => {
            let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
            if route.input.is_some() {
                format!(
                    "({input_arg}) => invoke<{output}, {}>(\"{}\", input)",
                    input_ts.unwrap(),
                    route.route
                )
            } else {
                format!("() => invoke<{output}>(\"{}\")", route.route)
            }
        }
        RouteKind::Stream => {
            let event = rust_to_ts(route.event.as_deref().unwrap_or("unknown"));
            let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
            if route.input.is_some() {
                format!(
                    "({input_arg}): NativeStream<{event}, {output}> => stream<{event}, {output}, {}>(\"{}\", input)",
                    input_ts.unwrap(),
                    route.route
                )
            } else {
                format!(
                    "(): NativeStream<{event}, {output}> => stream<{event}, {output}>(\"{}\")",
                    route.route
                )
            }
        }
        RouteKind::Channel => {
            let inbound = rust_to_ts(route.channel_in.as_deref().unwrap_or("unknown"));
            let outbound = rust_to_ts(route.channel_out.as_deref().unwrap_or("unknown"));
            if route.input.is_some() {
                format!(
                    "({input_arg}): Promise<NativeChannel<{inbound}, {outbound}>> => channel<{inbound}, {outbound}>(\"{}\", input)",
                    route.route
                )
            } else {
                format!(
                    "(): Promise<NativeChannel<{inbound}, {outbound}>> => channel<{inbound}, {outbound}>(\"{}\")",
                    route.route
                )
            }
        }
    }
}

fn render_js_route_method(route: &NativeRoute) -> String {
    let parts = route
        .route
        .split('.')
        .map(json_string)
        .collect::<Vec<_>>()
        .join(", ");
    match route.kind {
        RouteKind::Call => format!("call([{parts}])"),
        RouteKind::Stream => format!("stream([{parts}])"),
        RouteKind::Channel => format!("channel([{parts}])"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn type_string(ty: &syn::Type) -> String {
    quote::quote!(#ty).to_string()
}

fn matching_angle(input: &str, start: usize) -> Option<usize> {
    let mut depth = 0;
    for (idx, ch) in input.char_indices().skip_while(|(idx, _)| *idx < start) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn to_camel_case(name: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn safe_ts_property(name: &str) -> String {
    if name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        name.to_string()
    } else {
        json_string(name)
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_source(source: &str) -> NativeManifest {
        let ast = syn::parse_file(source).unwrap();
        let mut manifest = NativeManifest {
            routes: Vec::new(),
            structs: BTreeMap::new(),
        };
        for item in &ast.items {
            match item {
                syn::Item::Fn(item_fn) => {
                    if let Some((route, kind)) = parse_attr(&item_fn.attrs) {
                        manifest
                            .routes
                            .push(extract_route_info(&route, kind, item_fn));
                    }
                }
                syn::Item::Struct(item_struct) => {
                    let fields = extract_struct_fields(item_struct);
                    if !fields.is_empty() {
                        manifest
                            .structs
                            .insert(item_struct.ident.to_string(), fields);
                    }
                }
                _ => {}
            }
        }
        manifest.routes.sort_by(|a, b| a.route.cmp(&b.route));
        manifest
    }

    #[test]
    fn parses_native_call_and_private_struct() {
        let manifest = scan_source(
            r#"
            struct OpenDeviceInput {
                device_id: String,
                retry_count: Option<u32>,
            }

            #[lingxia::native("device.open")]
            pub async fn open_device(input: OpenDeviceInput) -> HostResult<()> { todo!() }
        "#,
        );
        let generated = render(&manifest, OutputKind::TypeScriptModule).unwrap();
        assert!(generated.contains("deviceId: string"));
        assert!(generated.contains("retryCount?: number"));
        assert!(generated.contains("invoke<void, OpenDeviceInput>"));
    }

    #[test]
    fn route_names_do_not_select_stream_or_channel_mode() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("demo.streamInfo")]
            pub fn stream_info() -> HostResult<String> { todo!() }

            #[lingxia::native("demo.channelState")]
            pub fn channel_state() -> HostResult<String> { todo!() }
        "#,
        );
        assert_eq!(manifest.routes[0].kind, RouteKind::Call);
        assert_eq!(manifest.routes[1].kind, RouteKind::Call);
    }

    #[test]
    fn parses_stream_and_channel_context_types() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("downloads.watch", stream)]
            pub async fn watch(ctx: crate::host::StreamContext<DownloadEvent, ()>) -> HostResult<()> { todo!() }

            #[lingxia::native("editor.session", channel)]
            pub async fn session(ctx: ChannelContext<EditorInput, EditorEvent>) -> HostResult<()> { todo!() }
        "#,
        );
        let watch = manifest
            .routes
            .iter()
            .find(|route| route.route == "downloads.watch")
            .unwrap();
        assert_eq!(watch.event.as_deref(), Some("DownloadEvent"));

        let session = manifest
            .routes
            .iter()
            .find(|route| route.route == "editor.session")
            .unwrap();
        assert_eq!(session.channel_in.as_deref(), Some("EditorInput"));
        assert_eq!(session.channel_out.as_deref(), Some("EditorEvent"));
    }

    #[test]
    fn generated_browser_js_uses_lingxia_bridge() {
        let mut manifest = NativeManifest {
            routes: Vec::new(),
            structs: BTreeMap::new(),
        };
        manifest.routes.push(NativeRoute {
            route: "downloads.list".to_string(),
            kind: RouteKind::Call,
            input: None,
            output: Some("DownloadsSnapshot".to_string()),
            event: None,
            channel_in: None,
            channel_out: None,
        });
        let generated = render(&manifest, OutputKind::BrowserGlobalJs).unwrap();
        assert!(generated.contains("global.native"));
        assert!(generated.contains("LingXiaBridge"));
        assert!(generated.contains("call([\"downloads\", \"list\"])"));
    }

    #[test]
    fn detects_route_prefix_conflicts() {
        let routes = vec![
            NativeRoute {
                route: "a.b".to_string(),
                kind: RouteKind::Call,
                input: None,
                output: None,
                event: None,
                channel_in: None,
                channel_out: None,
            },
            NativeRoute {
                route: "a.b.c".to_string(),
                kind: RouteKind::Call,
                input: None,
                output: None,
                event: None,
                channel_in: None,
                channel_out: None,
            },
        ];
        assert!(RouteNode::build(&routes).is_err());
    }
}
