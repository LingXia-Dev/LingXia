use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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
struct NativeStruct {
    fields: Vec<NativeField>,
}

#[derive(Debug, Clone)]
struct NativeField {
    name: String,
    ty: String,
    optional: bool,
}

#[derive(Debug, Default)]
struct NativeManifest {
    routes: Vec<NativeRoute>,
    structs: BTreeMap<String, NativeStruct>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    TypeScriptModule,
    BrowserGlobalJs,
}

pub(crate) fn generate_native_client_from_paths(rust_dir: &Path, out: &Path) -> Result<()> {
    generate_native_client(rust_dir, out)
}

fn generate_native_client(rust_dir: &Path, out: &Path) -> Result<()> {
    if !rust_dir.exists() {
        return Err(anyhow!(
            "Native Rust API directory not found: {}",
            rust_dir.display()
        ));
    }
    let manifest = scan_native_manifest(rust_dir)?;
    let generated = render_native_client(&manifest, output_kind(out))?;

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let changed = fs::read_to_string(out)
        .map(|old| old != generated)
        .unwrap_or(true);
    if changed {
        fs::write(out, generated).with_context(|| format!("Failed to write {}", out.display()))?;
        let label = out
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("native client");
        println!("  {} {} → {}", "✓".green(), label, out.display());
    }
    Ok(())
}

fn output_kind(out: &Path) -> OutputKind {
    match out.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("js") => OutputKind::BrowserGlobalJs,
        _ => OutputKind::TypeScriptModule,
    }
}

fn scan_native_manifest(rust_dir: &Path) -> Result<NativeManifest> {
    let mut manifest = NativeManifest::default();
    for file in rust_files(rust_dir)? {
        let source = fs::read_to_string(&file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        scan_structs(&source, &mut manifest.structs);
        scan_routes(&source, &file, &mut manifest.routes)?;
    }
    detect_route_collisions(&manifest.routes)?;
    manifest.routes.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(manifest)
}

fn rust_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_rust_files(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn scan_routes(source: &str, file: &Path, routes: &mut Vec<NativeRoute>) -> Result<()> {
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some((route, kind)) = parse_native_attr(line) {
            i += 1;
            while i < lines.len() && !lines[i].contains("fn ") {
                i += 1;
            }
            if i >= lines.len() {
                return Err(anyhow!(
                    "Native route `{}` in {} is not followed by a function",
                    route,
                    file.display()
                ));
            }
            let signature = collect_signature(&lines, i);
            routes.push(parse_route_signature(route, kind, &signature));
        }
        i += 1;
    }
    Ok(())
}

fn parse_native_attr(line: &str) -> Option<(String, RouteKind)> {
    let is_native = line.starts_with("#[lingxia::native") || line.starts_with("#[native");
    if !is_native {
        return None;
    }
    let first_quote = line.find('"')?;
    let rest = &line[first_quote + 1..];
    let second_quote = rest.find('"')?;
    let route = rest[..second_quote].to_string();
    let mode_args = rest[second_quote + 1..]
        .split(']')
        .next()
        .unwrap_or_default();
    let mode_args = mode_args
        .trim()
        .trim_start_matches(',')
        .trim()
        .trim_end_matches(')')
        .trim();
    let mode_tokens = split_top_level(mode_args, ',')
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    let kind = if mode_tokens.iter().any(|token| *token == "channel") {
        RouteKind::Channel
    } else if mode_tokens.iter().any(|token| *token == "stream") {
        RouteKind::Stream
    } else {
        RouteKind::Call
    };
    Some((route, kind))
}

fn collect_signature(lines: &[&str], start: usize) -> String {
    let mut signature = String::new();
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        signature.push_str(line);
        signature.push(' ');
        if line.contains('{') {
            break;
        }
        i += 1;
    }
    signature
}

fn parse_route_signature(route: String, kind: RouteKind, signature: &str) -> NativeRoute {
    let args = signature
        .find('(')
        .and_then(|start| matching_paren(signature, start).map(|end| &signature[start + 1..end]))
        .unwrap_or("");
    let arg_types = split_top_level(args, ',')
        .into_iter()
        .filter_map(|arg| arg.split_once(':').map(|(_, ty)| clean_type(ty)))
        .collect::<Vec<_>>();
    let input = arg_types.iter().find(|ty| is_payload_type(ty)).cloned();
    let output = parse_output_type(signature);

    let mut route_info = NativeRoute {
        route,
        kind: kind.clone(),
        input,
        output,
        event: None,
        channel_in: None,
        channel_out: None,
    };

    for ty in arg_types {
        if let Some(args) = generic_args(&ty, "StreamContext") {
            route_info.event = args
                .first()
                .cloned()
                .or_else(|| Some("unknown".to_string()));
            route_info.output = args.get(1).cloned().or_else(|| Some("void".to_string()));
        }
        if let Some(args) = generic_args(&ty, "ChannelContext") {
            route_info.channel_in = args
                .first()
                .cloned()
                .or_else(|| Some("unknown".to_string()));
            route_info.channel_out = args
                .get(1)
                .cloned()
                .or_else(|| route_info.channel_in.clone())
                .or_else(|| Some("unknown".to_string()));
        }
    }
    route_info
}

fn is_payload_type(ty: &str) -> bool {
    !ty.contains("LxApp")
        && !ty.contains("HostCancel")
        && !ty.contains("StreamContext")
        && !ty.contains("ChannelContext")
}

fn parse_output_type(signature: &str) -> Option<String> {
    let arrow = signature.find("->")?;
    let tail = signature[arrow + 2..]
        .split('{')
        .next()
        .unwrap_or("")
        .split("where")
        .next()
        .unwrap_or("")
        .trim();
    Some(unwrap_result_type(&clean_type(tail)))
}

fn unwrap_result_type(ty: &str) -> String {
    for wrapper in ["HostResult", "Result", "std::result::Result"] {
        if let Some(args) = generic_args(ty, wrapper) {
            return args.first().cloned().unwrap_or_else(|| "void".to_string());
        }
    }
    ty.to_string()
}

fn scan_structs(source: &str, structs: &mut BTreeMap<String, NativeStruct>) {
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        let Some(name) = parse_struct_name(line) else {
            i += 1;
            continue;
        };
        let mut body = Vec::new();
        i += 1;
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with('}') {
                break;
            }
            body.push(trimmed.to_string());
            i += 1;
        }
        let fields = body
            .into_iter()
            .filter_map(|line| parse_struct_field(&line))
            .collect::<Vec<_>>();
        structs.insert(name, NativeStruct { fields });
        i += 1;
    }
}

fn parse_struct_name(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("pub struct ")
        .or_else(|| line.strip_prefix("struct "))?;
    let name = rest
        .split(|c: char| c == '{' || c == '<' || c.is_whitespace())
        .next()?;
    (!name.is_empty()).then(|| name.to_string())
}

fn parse_struct_field(line: &str) -> Option<NativeField> {
    let rest = line.strip_prefix("pub ").unwrap_or(line);
    let (name, ty) = rest.split_once(':')?;
    let name = name.trim();
    if name.is_empty() || name.starts_with('#') || name.starts_with("//") {
        return None;
    }
    let mut ty = clean_type(ty.trim_end_matches(','));
    let optional = if let Some(args) = generic_args(&ty, "Option") {
        ty = args
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        true
    } else {
        false
    };
    Some(NativeField {
        name: to_camel_case(name),
        ty,
        optional,
    })
}

fn render_native_client(manifest: &NativeManifest, output_kind: OutputKind) -> Result<String> {
    match output_kind {
        OutputKind::TypeScriptModule => render_native_client_ts(manifest),
        OutputKind::BrowserGlobalJs => render_native_client_js(manifest),
    }
}

fn render_native_client_ts(manifest: &NativeManifest) -> Result<String> {
    let mut used_types = BTreeSet::new();
    for route in &manifest.routes {
        collect_type_ref(route.input.as_deref(), &mut used_types);
        collect_type_ref(route.output.as_deref(), &mut used_types);
        collect_type_ref(route.event.as_deref(), &mut used_types);
        collect_type_ref(route.channel_in.as_deref(), &mut used_types);
        collect_type_ref(route.channel_out.as_deref(), &mut used_types);
    }

    let mut out = String::new();
    out.push_str("// Generated by `lingxia build`. Do not edit by hand.\n");
    out.push_str("import { channel, invoke, stream } from \"@lingxia/bridge\";\n");
    out.push_str("import type { NativeChannel, NativeStream } from \"@lingxia/bridge\";\n\n");
    out.push_str("export type NativeVoid = void;\n\n");
    for ty in used_types {
        if is_builtin_ts_type(&ty) {
            continue;
        }
        if let Some(def) = manifest.structs.get(&ty) {
            out.push_str(&format!("export interface {ty} {{\n"));
            for field in &def.fields {
                out.push_str(&format!(
                    "  {}{}: {};\n",
                    field.name,
                    if field.optional { "?" } else { "" },
                    rust_type_to_ts(&field.ty)
                ));
            }
            out.push_str("}\n\n");
        } else {
            out.push_str(&format!("export type {ty} = unknown;\n\n"));
        }
    }

    let tree = build_route_tree(&manifest.routes)?;
    out.push_str("export const native = ");
    out.push_str(&render_route_node(&tree, 0));
    out.push_str(";\n");
    Ok(out)
}

fn render_native_client_js(manifest: &NativeManifest) -> Result<String> {
    let tree = build_route_tree(&manifest.routes)?;
    let mut out = String::new();
    out.push_str("// Generated by `lingxia build`. Do not edit by hand.\n");
    out.push_str("(function (global) {\n");
    out.push_str("  function bridge() {\n");
    out.push_str("    if (!global.LingXiaBridge) throw new Error('window.LingXiaBridge is not available');\n");
    out.push_str("    return global.LingXiaBridge;\n");
    out.push_str("  }\n");
    out.push_str("  function route(parts) {\n");
    out.push_str("    return 'host.' + parts.join('.');\n");
    out.push_str("  }\n");
    out.push_str("  function nativeError(error) {\n");
    out.push_str("    if (error && typeof error === 'object') {\n");
    out.push_str("      var code = typeof error.code === 'string' && error.code ? error.code : 'BRIDGE_INTERNAL_ERROR';\n");
    out.push_str("      var message = typeof error.message === 'string' && error.message ? error.message : 'Unknown error';\n");
    out.push_str("      var out = { code: code, message: message };\n");
    out.push_str("      if ('data' in error) out.data = error.data;\n");
    out.push_str("      return out;\n");
    out.push_str("    }\n");
    out.push_str("    return { code: 'BRIDGE_INTERNAL_ERROR', message: error instanceof Error ? error.message : String(error || 'Unknown error') };\n");
    out.push_str("  }\n");
    out.push_str("  function call(parts) {\n");
    out.push_str("    return function (input) {\n");
    out.push_str("      return bridge().call(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).catch(function (error) { return Promise.reject(nativeError(error)); });\n");
    out.push_str("    };\n");
    out.push_str("  }\n");
    out.push_str("  function stream(parts) {\n");
    out.push_str("    return function (input) {\n");
    out.push_str("      var handle = bridge().stream(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host', timeoutMs: 0 });\n");
    out.push_str("      var eventListeners = [];\n");
    out.push_str("      var errorListeners = [];\n");
    out.push_str("      handle.on('data', function (event) { eventListeners.slice().forEach(function (listener) { listener(event); }); });\n");
    out.push_str("      handle.on('error', function (error) { var normalized = nativeError(error); errorListeners.slice().forEach(function (listener) { listener(normalized); }); });\n");
    out.push_str("      return {\n");
    out.push_str("        onEvent: function (listener) { eventListeners.push(listener); return function () { eventListeners = eventListeners.filter(function (item) { return item !== listener; }); }; },\n");
    out.push_str("        onError: function (listener) { errorListeners.push(listener); return function () { errorListeners = errorListeners.filter(function (item) { return item !== listener; }); }; },\n");
    out.push_str("        result: handle.result.catch(function (error) { return Promise.reject(nativeError(error)); }),\n");
    out.push_str("        cancel: function () { handle.cancel(); }\n");
    out.push_str("      };\n");
    out.push_str("    };\n");
    out.push_str("  }\n");
    out.push_str("  function channel(parts) {\n");
    out.push_str("    return function (input) {\n");
    out.push_str("      return bridge().channel.open(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).then(function (handle) {\n");
    out.push_str("        var messageListeners = [];\n");
    out.push_str("        var closeListeners = [];\n");
    out.push_str("        handle.on('data', function (message) { messageListeners.slice().forEach(function (listener) { listener(message); }); });\n");
    out.push_str("        handle.on('close', function (code, reason) { var event = { code: code, reason: reason }; closeListeners.slice().forEach(function (listener) { listener(event); }); });\n");
    out.push_str("        return {\n");
    out.push_str("          send: function (message) { handle.send(message); },\n");
    out.push_str("          onMessage: function (listener) { messageListeners.push(listener); return function () { messageListeners = messageListeners.filter(function (item) { return item !== listener; }); }; },\n");
    out.push_str("          onClose: function (listener) { closeListeners.push(listener); return function () { closeListeners = closeListeners.filter(function (item) { return item !== listener; }); }; },\n");
    out.push_str("          close: function (code, reason) { handle.close(code, reason); }\n");
    out.push_str("        };\n");
    out.push_str(
        "      }).catch(function (error) { return Promise.reject(nativeError(error)); });\n",
    );
    out.push_str("    };\n");
    out.push_str("  }\n");
    out.push_str("  global.native = ");
    out.push_str(&render_js_route_node(&tree, &mut Vec::new(), 2));
    out.push_str(";\n");
    out.push_str("})(window);\n");
    Ok(out)
}

#[derive(Default)]
struct RouteNode {
    children: BTreeMap<String, RouteNode>,
    route: Option<NativeRoute>,
}

fn build_route_tree(routes: &[NativeRoute]) -> Result<RouteNode> {
    let mut root = RouteNode::default();
    for route in routes {
        let parts = route.route.split('.').collect::<Vec<_>>();
        if parts.len() < 2 || parts.iter().any(|part| part.trim().is_empty()) {
            return Err(anyhow!("Invalid native route `{}`", route.route));
        }
        let mut node = &mut root;
        for part in parts {
            if node.route.is_some() {
                return Err(anyhow!(
                    "Native route `{}` conflicts with route prefix",
                    route.route
                ));
            }
            node = node.children.entry(part.to_string()).or_default();
        }
        if !node.children.is_empty() {
            return Err(anyhow!(
                "Native route `{}` conflicts with existing route namespace",
                route.route
            ));
        }
        node.route = Some(route.clone());
    }
    Ok(root)
}

fn render_route_node(node: &RouteNode, indent: usize) -> String {
    if let Some(route) = &node.route {
        return render_route_method(route);
    }
    let pad = " ".repeat(indent);
    let child_pad = " ".repeat(indent + 2);
    let mut out = String::from("{\n");
    for (name, child) in &node.children {
        out.push_str(&format!(
            "{child_pad}{}: {},\n",
            safe_ts_property(name),
            render_route_node(child, indent + 2)
        ));
    }
    out.push_str(&format!("{pad}}}"));
    out
}

fn render_js_route_node(node: &RouteNode, path_parts: &mut Vec<String>, indent: usize) -> String {
    if let Some(route) = &node.route {
        let parts = route
            .route
            .split('.')
            .map(|part| serde_json::to_string(part).unwrap_or_else(|_| "\"\"".to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        return match route.kind {
            RouteKind::Call => format!("call([{parts}])"),
            RouteKind::Stream => format!("stream([{parts}])"),
            RouteKind::Channel => format!("channel([{parts}])"),
        };
    }

    let pad = " ".repeat(indent);
    let child_pad = " ".repeat(indent + 2);
    let mut out = String::from("{\n");
    for (name, child) in &node.children {
        path_parts.push(name.clone());
        out.push_str(&format!(
            "{child_pad}{}: {},\n",
            safe_ts_property(name),
            render_js_route_node(child, path_parts, indent + 2)
        ));
        path_parts.pop();
    }
    out.push_str(&format!("{pad}}}"));
    out
}

fn render_route_method(route: &NativeRoute) -> String {
    let input = route.input.as_deref();
    let input_ts = input.map(rust_type_to_ts);
    let input_arg = input_ts
        .as_ref()
        .map(|ty| format!("input: {ty}"))
        .unwrap_or_default();
    match route.kind {
        RouteKind::Call => {
            let output = rust_type_to_ts(route.output.as_deref().unwrap_or("void"));
            if input.is_some() {
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
            let event = rust_type_to_ts(route.event.as_deref().unwrap_or("unknown"));
            let output = rust_type_to_ts(route.output.as_deref().unwrap_or("void"));
            if input.is_some() {
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
            let inbound = rust_type_to_ts(route.channel_in.as_deref().unwrap_or("unknown"));
            let outbound = rust_type_to_ts(route.channel_out.as_deref().unwrap_or("unknown"));
            if input.is_some() {
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

fn collect_type_ref(ty: Option<&str>, out: &mut BTreeSet<String>) {
    let Some(ty) = ty.map(str::trim).filter(|ty| !ty.is_empty()) else {
        return;
    };
    let cleaned = clean_type(ty);
    for wrapper in ["Option", "Vec"] {
        if let Some(args) = generic_args(&cleaned, wrapper) {
            collect_type_ref(args.first().map(String::as_str), out);
            return;
        }
    }
    for wrapper in ["HashMap", "BTreeMap"] {
        if let Some(args) = generic_args(&cleaned, wrapper) {
            collect_type_ref(args.get(1).map(String::as_str), out);
            return;
        }
    }

    let base = cleaned
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .split("::")
        .last()
        .unwrap_or(&cleaned)
        .trim();
    if !is_builtin_ts_type(base) && base.chars().next().is_some_and(|ch| ch.is_uppercase()) {
        out.insert(base.to_string());
    }
}

fn rust_type_to_ts(ty: &str) -> String {
    let ty = clean_type(ty);
    if ty == "()" || ty == "void" {
        return "void".to_string();
    }
    if let Some(args) = generic_args(&ty, "Option") {
        return args
            .first()
            .map(|inner| rust_type_to_ts(inner))
            .unwrap_or_else(|| "unknown".to_string());
    }
    if let Some(args) = generic_args(&ty, "Vec") {
        return format!(
            "{}[]",
            args.first()
                .map(|inner| rust_type_to_ts(inner))
                .unwrap_or_else(|| "unknown".to_string())
        );
    }
    for map in ["HashMap", "BTreeMap"] {
        if let Some(args) = generic_args(&ty, map) {
            let value = args
                .get(1)
                .map(|inner| rust_type_to_ts(inner))
                .unwrap_or_else(|| "unknown".to_string());
            return format!("Record<string, {value}>");
        }
    }
    match ty.trim_start_matches('&').split("::").last().unwrap_or(&ty) {
        "String" | "str" => "string".to_string(),
        "bool" => "boolean".to_string(),
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize" | "f32"
        | "f64" => "number".to_string(),
        "Value" | "JsonValue" => "unknown".to_string(),
        other => other.to_string(),
    }
}

fn is_builtin_ts_type(ty: &str) -> bool {
    matches!(
        rust_type_to_ts(ty).as_str(),
        "string" | "boolean" | "number" | "unknown" | "void"
    )
}

fn clean_type(ty: &str) -> String {
    ty.trim()
        .trim_end_matches(',')
        .trim()
        .trim_start_matches("mut ")
        .to_string()
}

fn generic_args(ty: &str, wrapper: &str) -> Option<Vec<String>> {
    let ty = ty.trim();
    let pos = ty
        .rfind(&format!("{wrapper}<"))
        .or_else(|| ty.find(&format!("{wrapper}<")))?;
    let start = pos + wrapper.len();
    if ty.as_bytes().get(start) != Some(&b'<') {
        return None;
    }
    let end = matching_angle(ty, start)?;
    Some(
        split_top_level(&ty[start + 1..end], ',')
            .into_iter()
            .map(clean_type)
            .collect(),
    )
}

fn split_top_level(input: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth_angle = 0;
    let mut depth_paren = 0;
    for (idx, ch) in input.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            _ if ch == sep && depth_angle == 0 && depth_paren == 0 => {
                parts.push(input[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn matching_paren(input: &str, start: usize) -> Option<usize> {
    let mut depth = 0;
    for (idx, ch) in input.char_indices().skip_while(|(idx, _)| *idx < start) {
        match ch {
            '(' => depth += 1,
            ')' => {
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
        serde_json::to_string(name).unwrap_or_else(|_| "\"invalid\"".to_string())
    }
}

fn detect_route_collisions(routes: &[NativeRoute]) -> Result<()> {
    let mut seen = HashSet::new();
    for route in routes {
        if !seen.insert(route.route.clone()) {
            return Err(anyhow!("Duplicate native route `{}`", route.route));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_call_and_struct() {
        let source = r#"
            pub struct OpenDeviceInput {
                pub device_id: String,
                pub retry_count: Option<u32>,
            }

            #[lingxia::native("device.open")]
            pub async fn open_device(input: OpenDeviceInput) -> HostResult<()> { todo!() }
        "#;
        let mut manifest = NativeManifest::default();
        scan_structs(source, &mut manifest.structs);
        scan_routes(source, Path::new("lib.rs"), &mut manifest.routes).unwrap();
        let generated = render_native_client(&manifest, OutputKind::TypeScriptModule).unwrap();
        assert!(generated.contains("deviceId: string"));
        assert!(generated.contains("retryCount?: number"));
        assert!(generated.contains("device:"));
        assert!(generated.contains("open:"));
        assert!(generated.contains("invoke<void, OpenDeviceInput>"));
    }

    #[test]
    fn parses_stream_context_types() {
        let source = r#"
            #[lingxia::native("downloads.watch", stream)]
            pub async fn watch(ctx: StreamContext<DownloadEvent, ()>) -> HostResult<()> { todo!() }
        "#;
        let mut routes = Vec::new();
        scan_routes(source, Path::new("lib.rs"), &mut routes).unwrap();
        assert_eq!(routes[0].event.as_deref(), Some("DownloadEvent"));
        assert_eq!(routes[0].output.as_deref(), Some("()"));
    }

    #[test]
    fn route_names_do_not_select_stream_or_channel_mode() {
        let source = r#"
            #[lingxia::native("demo.streamInfo")]
            pub fn stream_info() -> HostResult<String> { todo!() }

            #[lingxia::native("demo.channelState")]
            pub fn channel_state() -> HostResult<String> { todo!() }
        "#;
        let mut routes = Vec::new();
        scan_routes(source, Path::new("lib.rs"), &mut routes).unwrap();
        assert_eq!(routes[0].kind, RouteKind::Call);
        assert_eq!(routes[1].kind, RouteKind::Call);
    }

    #[test]
    fn parses_channel_context_direction() {
        let source = r#"
            #[lingxia::native("editor.session", channel)]
            pub async fn session(ctx: ChannelContext<EditorInput, EditorEvent>) -> HostResult<()> { todo!() }
        "#;
        let mut routes = Vec::new();
        scan_routes(source, Path::new("lib.rs"), &mut routes).unwrap();
        assert_eq!(routes[0].channel_in.as_deref(), Some("EditorInput"));
        assert_eq!(routes[0].channel_out.as_deref(), Some("EditorEvent"));

        let mut manifest = NativeManifest::default();
        manifest.routes = routes;
        let generated = render_native_client(&manifest, OutputKind::TypeScriptModule).unwrap();
        assert!(generated.contains("NativeChannel<EditorInput, EditorEvent>"));
    }

    #[test]
    fn generated_browser_js_uses_lingxia_bridge() {
        let mut manifest = NativeManifest::default();
        manifest.routes.push(NativeRoute {
            route: "downloads.list".to_string(),
            kind: RouteKind::Call,
            input: None,
            output: Some("DownloadsSnapshot".to_string()),
            event: None,
            channel_in: None,
            channel_out: None,
        });
        let generated = render_native_client(&manifest, OutputKind::BrowserGlobalJs).unwrap();
        assert!(generated.contains("global.native"));
        assert!(generated.contains("LingXiaBridge"));
        assert!(generated.contains("call([\"downloads\", \"list\"])"));
        assert!(generated.contains("onEvent"));
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
        assert!(build_route_tree(&routes).is_err());
    }

    #[test]
    fn collects_nested_type_references() {
        let mut out = BTreeSet::new();
        collect_type_ref(Some("Option<Vec<DownloadEvent>>"), &mut out);
        assert!(out.contains("DownloadEvent"));
    }
}
