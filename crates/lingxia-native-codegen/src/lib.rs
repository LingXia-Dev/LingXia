//! Code generation for `#[lingxia::native]` host handlers.
//!
//! Scans Rust source files for public `#[lingxia::native("route", ...)]` handlers
//! and their DTO types (`struct`, `enum`, `type`), then emits one of:
//!
//! - `.logic.ts` — Logic runtime TypeScript client (with `sync`, `NativeTask`, `NativeChannel`, `createNative(pageId)`)
//! - `.view.ts` / `.ts` — WebView View TypeScript client (`@lingxia/bridge` adapted to the same unified shapes)
//! - `.js` — Browser global JS client (same unified shapes)
//! - `.rs` — Rust auto-register module (`mod __lingxia_native { pub fn install() }`)

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Expr, Fields, FnArg, ItemEnum, ItemFn, ItemStruct, Lit, Meta, ReturnType, Token,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Emit the WebView View client at `out` (`.ts`, or browser-global `.js`) and the Logic client
/// `native.logic.ts` beside it, for every `#[native]` handler discovered under `rust_dir`.
/// If no handlers are found, both files are removed.
pub fn generate_ts_client(rust_dir: &Path, out: &Path) -> Result<()> {
    let kind = output_kind(out);
    if !matches!(
        kind,
        OutputKind::ViewTypeScript | OutputKind::BrowserGlobalJs
    ) {
        bail!(
            "native client output must be a View `.ts` or `.js` file: {}",
            out.display()
        );
    }
    let logic_out = out
        .parent()
        .map(|dir| dir.join("native.logic.ts"))
        .ok_or_else(|| anyhow!("native client output has no directory: {}", out.display()))?;
    write_artifact(rust_dir, out, kind)?;
    write_artifact(rust_dir, &logic_out, OutputKind::LogicTypeScript)
}

/// Emit the Rust auto-register module that wraps every discovered handler's
/// `register_host_entry(...)` call inside `mod __lingxia_native { pub fn install() }`.
/// `out` must end in `.rs`.
pub fn generate_rust_registry(rust_dir: &Path, out: &Path) -> Result<()> {
    let kind = output_kind(out);
    debug_assert_eq!(kind, OutputKind::RustModule);
    write_artifact(rust_dir, out, kind)
}

fn write_if_changed(out: &Path, generated: &str) -> Result<()> {
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

fn write_artifact(rust_dir: &Path, out: &Path, kind: OutputKind) -> Result<()> {
    if !rust_dir.exists() {
        return Err(anyhow!(
            "Native Rust API directory not found: {}",
            rust_dir.display()
        ));
    }
    let manifest = scan(rust_dir)?;
    if manifest.routes.is_empty() && kind != OutputKind::RustModule {
        let _ = fs::remove_file(out);
        return Ok(());
    }
    let generated = render(&manifest, kind)?;
    write_if_changed(out, &generated)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    LogicTypeScript,
    ViewTypeScript,
    BrowserGlobalJs,
    RustModule,
}

fn output_kind(out: &Path) -> OutputKind {
    let name = out
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".logic.ts") {
        return OutputKind::LogicTypeScript;
    }
    match out.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("js") => OutputKind::BrowserGlobalJs,
        Some(ext) if ext.eq_ignore_ascii_case("rs") => OutputKind::RustModule,
        _ => OutputKind::ViewTypeScript,
    }
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteKind {
    Call,
    SyncCall,
    Stream,
    Channel,
}

#[derive(Debug, Clone)]
struct SourceLoc {
    file: String,
    line: usize,
}

impl SourceLoc {
    fn display(&self) -> String {
        if self.line > 0 {
            format!("{}:{}", self.file, self.line)
        } else {
            self.file.clone()
        }
    }
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
    fn_ident: String,
    loc: SourceLoc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenameRule {
    None,
    CamelCase,
    SnakeCase,
    PascalCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
    Lowercase,
    Uppercase,
}

impl RenameRule {
    fn parse(value: &str) -> Self {
        match value {
            "camelCase" => Self::CamelCase,
            "snake_case" => Self::SnakeCase,
            "PascalCase" => Self::PascalCase,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnakeCase,
            "kebab-case" => Self::KebabCase,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebabCase,
            "lowercase" => Self::Lowercase,
            "UPPERCASE" => Self::Uppercase,
            _ => Self::None,
        }
    }

    fn apply(self, ident: &str) -> String {
        match self {
            Self::None => ident.to_string(),
            Self::CamelCase => to_camel_case(ident),
            Self::SnakeCase => to_snake_case(ident),
            Self::PascalCase => to_pascal_case(ident),
            Self::ScreamingSnakeCase => to_snake_case(ident).to_ascii_uppercase(),
            Self::KebabCase => to_snake_case(ident).replace('_', "-"),
            Self::ScreamingKebabCase => to_snake_case(ident).replace('_', "-").to_ascii_uppercase(),
            Self::Lowercase => ident.to_ascii_lowercase(),
            Self::Uppercase => ident.to_ascii_uppercase(),
        }
    }
}

#[derive(Debug, Clone)]
enum EnumTagging {
    External,
    Internal { tag: String },
    Adjacent { tag: String, content: String },
    Untagged,
}

#[derive(Debug, Clone)]
struct StructField {
    name: String,
    ty: String,
    optional: bool,
    flatten: bool,
}

#[derive(Debug, Clone)]
enum StructDefKind {
    Named(Vec<StructField>),
    Tuple(Vec<String>),
    Unit,
}

#[derive(Debug, Clone)]
struct StructDef {
    kind: StructDefKind,
    loc: SourceLoc,
}

#[derive(Debug, Clone)]
enum VariantPayload {
    Unit,
    Newtype(String),
    Tuple(Vec<String>),
    Struct(Vec<StructField>),
}

#[derive(Debug, Clone)]
struct EnumVariantDef {
    wire_name: String,
    payload: VariantPayload,
}

#[derive(Debug, Clone)]
struct EnumDef {
    tagging: EnumTagging,
    variants: Vec<EnumVariantDef>,
    loc: SourceLoc,
}

#[derive(Debug, Clone)]
struct TypeAliasDef {
    target: String,
    loc: SourceLoc,
}

#[derive(Debug, Default)]
struct NativeManifest {
    routes: Vec<NativeRoute>,
    structs: BTreeMap<String, StructDef>,
    enums: BTreeMap<String, EnumDef>,
    aliases: BTreeMap<String, TypeAliasDef>,
}

// ---------------------------------------------------------------------------
// Scanner (syn-based)
// ---------------------------------------------------------------------------

fn scan(src_dir: &Path) -> Result<NativeManifest> {
    let mut manifest = NativeManifest::default();
    let mut files = Vec::new();
    collect_rs_files(src_dir, &mut files).map_err(|e| anyhow!("scan: {e}"))?;
    files.sort();

    for file in &files {
        let source =
            fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
        let file_label = file
            .strip_prefix(src_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        scan_file_into(&source, &file_label, &mut manifest)?;
    }

    let mut seen = BTreeSet::new();
    for r in &manifest.routes {
        if !seen.insert(r.route.clone()) {
            return Err(anyhow!("duplicate native route `{}`", r.route));
        }
    }

    manifest.routes.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(manifest)
}

fn find_item_line(source: &str, keyword: &str, ident: &str) -> usize {
    let needle = format!("{keyword} {ident}");
    source
        .lines()
        .enumerate()
        .find_map(|(idx, line)| line.contains(&needle).then_some(idx + 1))
        .unwrap_or(0)
}

fn scan_file_into(source: &str, file_label: &str, manifest: &mut NativeManifest) -> Result<()> {
    let ast = syn::parse_file(source).with_context(|| format!("parse {file_label}"))?;
    scan_items(&ast.items, source, file_label, manifest);
    Ok(())
}

fn scan_items(items: &[syn::Item], source: &str, file_label: &str, manifest: &mut NativeManifest) {
    for item in items {
        match item {
            syn::Item::Fn(item_fn) => {
                if let Some((route, kind)) = parse_attr(&item_fn.attrs) {
                    let ident = item_fn.sig.ident.to_string();
                    let loc = SourceLoc {
                        file: file_label.to_string(),
                        line: find_item_line(source, "fn", &ident),
                    };
                    manifest
                        .routes
                        .push(extract_route_info(&route, kind, item_fn, loc));
                }
            }
            syn::Item::Struct(item_struct) => {
                let ident = item_struct.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "struct", &ident),
                };
                if let Some(def) = extract_struct_def(item_struct, loc) {
                    manifest.structs.insert(ident, def);
                }
            }
            syn::Item::Enum(item_enum) => {
                let ident = item_enum.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "enum", &ident),
                };
                if let Some(def) = extract_enum_def(item_enum, loc) {
                    manifest.enums.insert(ident, def);
                }
            }
            syn::Item::Type(item_type) => {
                let ident = item_type.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "type", &ident),
                };
                manifest.aliases.insert(
                    ident,
                    TypeAliasDef {
                        target: type_string(&item_type.ty).replace(' ', ""),
                        loc,
                    },
                );
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, sub_items)) = &item_mod.content {
                    scan_items(sub_items, source, file_label, manifest);
                }
            }
            _ => {}
        }
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
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

fn parse_attr(attrs: &[Attribute]) -> Option<(String, RouteKind)> {
    for attr in attrs {
        if !is_public_native_attr(attr) {
            continue;
        }
        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        let args = attr
            .meta
            .require_list()
            .ok()?
            .parse_args_with(parser)
            .ok()?;
        let Expr::Lit(route) = args.first()? else {
            return None;
        };
        let Lit::Str(route) = &route.lit else {
            return None;
        };
        let kind = if args.iter().skip(1).any(is_channel_flag) {
            RouteKind::Channel
        } else if args.iter().skip(1).any(is_stream_flag) {
            RouteKind::Stream
        } else if args.iter().skip(1).any(is_sync_flag) {
            RouteKind::SyncCall
        } else {
            RouteKind::Call
        };
        return Some((route.value(), kind));
    }
    None
}

fn is_public_native_attr(attr: &Attribute) -> bool {
    let path = attr.path();
    if path.is_ident("native") {
        return true;
    }
    let mut segments = path.segments.iter().rev();
    matches!(
        (segments.next(), segments.next()),
        (Some(last), Some(prefix)) if prefix.ident == "lingxia" && last.ident == "native"
    )
}

fn is_stream_flag(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident("stream"))
}

fn is_channel_flag(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident("channel"))
}

fn is_sync_flag(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident("sync"))
}

fn extract_route_info(
    route: &str,
    kind: RouteKind,
    item_fn: &ItemFn,
    loc: SourceLoc,
) -> NativeRoute {
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

        // Dispatch supplies these; they are never part of the JSON input.
        if ty_str.contains("LxApp")
            || ty_str.contains("HostCancel")
            || ty_str.contains("HostInvocationContext")
            || ty_str.contains("PageState")
        {
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
        fn_ident: item_fn.sig.ident.to_string(),
        loc,
    }
}

// ---------------------------------------------------------------------------
// Serde attribute parsing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SerdeContainerAttrs {
    rename_all: Option<RenameRule>,
    tag: Option<String>,
    content: Option<String>,
    untagged: bool,
}

#[derive(Default)]
struct SerdeMemberAttrs {
    rename: Option<String>,
    default: bool,
    skip: bool,
    skip_serializing_if: bool,
    flatten: bool,
}

fn parse_serde_container_attrs(attrs: &[Attribute]) -> SerdeContainerAttrs {
    let mut out = SerdeContainerAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.rename_all = Some(RenameRule::parse(&s.value()));
                }
            } else if meta.path.is_ident("tag") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.tag = Some(s.value());
                }
            } else if meta.path.is_ident("content") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.content = Some(s.value());
                }
            } else if meta.path.is_ident("untagged") {
                out.untagged = true;
            } else if meta.input.peek(Token![=]) {
                let _ = meta.value().and_then(|v| v.parse::<Expr>());
            } else if meta.input.peek(syn::token::Paren) {
                let _ = meta.parse_nested_meta(|_| Ok(()));
            }
            Ok(())
        });
    }
    out
}

fn parse_serde_member_attrs(attrs: &[Attribute]) -> SerdeMemberAttrs {
    let mut out = SerdeMemberAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.rename = Some(s.value());
                }
            } else if meta.path.is_ident("default") {
                out.default = true;
                if meta.input.peek(Token![=]) {
                    let _ = meta.value().and_then(|v| v.parse::<Expr>());
                }
            } else if meta.path.is_ident("skip")
                || meta.path.is_ident("skip_serializing")
                || meta.path.is_ident("skip_deserializing")
            {
                if meta.path.is_ident("skip") {
                    out.skip = true;
                }
            } else if meta.path.is_ident("skip_serializing_if") {
                out.skip_serializing_if = true;
                if meta.input.peek(Token![=]) {
                    let _ = meta.value().and_then(|v| v.parse::<Expr>());
                }
            } else if meta.path.is_ident("flatten") {
                out.flatten = true;
            } else if meta.input.peek(Token![=]) {
                let _ = meta.value().and_then(|v| v.parse::<Expr>());
            } else if meta.input.peek(syn::token::Paren) {
                let _ = meta.parse_nested_meta(|_| Ok(()));
            }
            Ok(())
        });
    }
    out
}

fn extract_named_fields(fields: &syn::FieldsNamed, default_rule: RenameRule) -> Vec<StructField> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let serde = parse_serde_member_attrs(&field.attrs);
            if serde.skip {
                return None;
            }
            let raw_name = field.ident.as_ref()?.to_string();
            let name = serde
                .rename
                .unwrap_or_else(|| default_rule.apply(&raw_name));
            let ty_str = type_string(&field.ty).replace(' ', "");
            let optional =
                ty_str.starts_with("Option<") || serde.default || serde.skip_serializing_if;
            Some(StructField {
                name,
                ty: ty_str,
                optional,
                flatten: serde.flatten,
            })
        })
        .collect()
}

fn extract_struct_def(item: &ItemStruct, loc: SourceLoc) -> Option<StructDef> {
    let container = parse_serde_container_attrs(&item.attrs);
    // Preserve LingXia's existing camelCase convention when no explicit serde rename_all is set.
    let rule = container.rename_all.unwrap_or(RenameRule::CamelCase);
    let kind = match &item.fields {
        Fields::Named(named) => StructDefKind::Named(extract_named_fields(named, rule)),
        Fields::Unnamed(unnamed) => {
            let elems = unnamed
                .unnamed
                .iter()
                .map(|f| type_string(&f.ty).replace(' ', ""))
                .collect();
            StructDefKind::Tuple(elems)
        }
        Fields::Unit => StructDefKind::Unit,
    };
    Some(StructDef { kind, loc })
}

fn extract_enum_def(item: &ItemEnum, loc: SourceLoc) -> Option<EnumDef> {
    let container = parse_serde_container_attrs(&item.attrs);
    let rule = container.rename_all.unwrap_or(RenameRule::None);
    let tagging = if container.untagged {
        EnumTagging::Untagged
    } else if let (Some(tag), Some(content)) = (container.tag.clone(), container.content.clone()) {
        EnumTagging::Adjacent { tag, content }
    } else if let Some(tag) = container.tag {
        EnumTagging::Internal { tag }
    } else {
        EnumTagging::External
    };

    let mut variants = Vec::new();
    for variant in &item.variants {
        let member = parse_serde_member_attrs(&variant.attrs);
        if member.skip {
            continue;
        }
        let raw_name = variant.ident.to_string();
        let wire_name = member.rename.unwrap_or_else(|| rule.apply(&raw_name));
        let payload = match &variant.fields {
            Fields::Unit => VariantPayload::Unit,
            Fields::Unnamed(unnamed) => {
                let elems: Vec<String> = unnamed
                    .unnamed
                    .iter()
                    .map(|f| type_string(&f.ty).replace(' ', ""))
                    .collect();
                if elems.len() == 1 {
                    VariantPayload::Newtype(elems.into_iter().next().unwrap())
                } else {
                    VariantPayload::Tuple(elems)
                }
            }
            Fields::Named(named) => {
                let field_rule =
                    has_serde_rename_all_fields(&variant.attrs).unwrap_or(RenameRule::CamelCase);
                VariantPayload::Struct(extract_named_fields(named, field_rule))
            }
        };
        variants.push(EnumVariantDef { wire_name, payload });
    }
    Some(EnumDef {
        tagging,
        variants,
        loc,
    })
}

fn has_serde_rename_all_fields(attrs: &[Attribute]) -> Option<RenameRule> {
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        if let Meta::List(list) = &attr.meta {
            let text = list.tokens.to_string();
            if let Some(rest) = text.split("rename_all").nth(1)
                && let Some(start) = rest.find('"')
                && let Some(end) = rest[start + 1..].find('"')
            {
                return Some(RenameRule::parse(&rest[start + 1..start + 1 + end]));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Type parsing & validation (strict: unknown types fail codegen)
// ---------------------------------------------------------------------------

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
    let Some(body) = ty.get(start + 1..end) else {
        return vec![];
    };
    split_top_level(body)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut angle = 0i32;
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => angle += 1,
            '>' => angle -= 1,
            '(' => paren += 1,
            ')' => paren -= 1,
            '[' => bracket += 1,
            ']' => bracket -= 1,
            ',' if angle == 0 && paren == 0 && bracket == 0 => {
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

fn is_builtin_rust_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "str"
            | "char"
            | "bool"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
            | "()"
            | "void"
            | "Value"
            | "JsonValue"
            | "unknown"
            | "string"
            | "number"
            | "boolean"
    )
}

fn collect_refs_from_type(ty: &str, out: &mut BTreeSet<String>) {
    let cleaned = ty.trim().trim_start_matches('&').trim_start_matches("mut ");
    if cleaned.is_empty() || cleaned == "void" || cleaned == "()" {
        return;
    }
    // Tuple `(A, B, ...)`
    if let Some(inner) = cleaned.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        for elem in split_top_level(inner) {
            collect_refs_from_type(&elem, out);
        }
        return;
    }
    // Slice / fixed array `[T]` or `[T; N]`
    if let Some(inner) = cleaned.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let elem = inner.split(';').next().unwrap_or(inner);
        collect_refs_from_type(elem, out);
        return;
    }
    for wrapper in &[
        "Option", "Vec", "HashSet", "BTreeSet", "Box", "Arc", "Rc", "Cow",
    ] {
        let args = extract_generic_args(cleaned, wrapper);
        if !args.is_empty() {
            // `Cow<'_, T>` has a lifetime arg first.
            let target = args
                .iter()
                .find(|a| !a.starts_with('\''))
                .map(String::as_str);
            if let Some(t) = target {
                collect_refs_from_type(t, out);
            }
            return;
        }
    }
    for wrapper in &["HashMap", "BTreeMap"] {
        let args = extract_generic_args(cleaned, wrapper);
        if !args.is_empty() {
            if let Some(k) = args.first() {
                collect_refs_from_type(k, out);
            }
            if let Some(v) = args.get(1) {
                collect_refs_from_type(v, out);
            }
            return;
        }
    }
    let base = type_basename(cleaned);
    if !is_builtin_rust_type(base) && !base.is_empty() {
        out.insert(base.to_string());
    }
}

/// Collects every transitively referenced user DTO type and fails if any type is unresolved.
fn collect_and_validate_types(manifest: &NativeManifest) -> Result<BTreeSet<String>> {
    let mut pending: Vec<(String, String, SourceLoc)> = Vec::new();
    for r in &manifest.routes {
        for ty in [
            r.input.as_deref(),
            r.output.as_deref(),
            r.event.as_deref(),
            r.channel_in.as_deref(),
            r.channel_out.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let mut refs = BTreeSet::new();
            collect_refs_from_type(ty, &mut refs);
            for name in refs {
                pending.push((name, format!("route `{}`", r.route), r.loc.clone()));
            }
        }
    }

    let mut resolved = BTreeSet::new();
    while let Some((name, owner, loc)) = pending.pop() {
        if resolved.contains(&name) {
            continue;
        }
        if let Some(def) = manifest.structs.get(&name) {
            resolved.insert(name.clone());
            let mut child_refs = BTreeSet::new();
            match &def.kind {
                StructDefKind::Named(fields) => {
                    for f in fields {
                        collect_refs_from_type(&f.ty, &mut child_refs);
                    }
                }
                StructDefKind::Tuple(elems) => {
                    for e in elems {
                        collect_refs_from_type(e, &mut child_refs);
                    }
                }
                StructDefKind::Unit => {}
            }
            for child in child_refs {
                pending.push((child, format!("struct `{name}`"), def.loc.clone()));
            }
        } else if let Some(def) = manifest.enums.get(&name) {
            resolved.insert(name.clone());
            let mut child_refs = BTreeSet::new();
            for v in &def.variants {
                match &v.payload {
                    VariantPayload::Unit => {}
                    VariantPayload::Newtype(t) => collect_refs_from_type(t, &mut child_refs),
                    VariantPayload::Tuple(ts) => {
                        for t in ts {
                            collect_refs_from_type(t, &mut child_refs);
                        }
                    }
                    VariantPayload::Struct(fields) => {
                        for f in fields {
                            collect_refs_from_type(&f.ty, &mut child_refs);
                        }
                    }
                }
            }
            for child in child_refs {
                pending.push((child, format!("enum `{name}`"), def.loc.clone()));
            }
        } else if let Some(def) = manifest.aliases.get(&name) {
            resolved.insert(name.clone());
            let mut child_refs = BTreeSet::new();
            collect_refs_from_type(&def.target, &mut child_refs);
            for child in child_refs {
                pending.push((child, format!("type `{name}`"), def.loc.clone()));
            }
        } else {
            bail!(
                "{}: unsupported or unresolved native DTO type `{name}` referenced by {owner}; define it as a struct, enum, or type alias in the native crate",
                loc.display()
            );
        }
    }
    Ok(resolved)
}

fn rust_to_ts(ty: &str) -> String {
    let cleaned = ty.trim().trim_start_matches('&').trim_start_matches("mut ");
    if cleaned.is_empty() || cleaned == "()" || cleaned == "void" {
        return "void".to_string();
    }
    if let Some(inner) = cleaned.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<String> = split_top_level(inner)
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .map(|e| rust_to_ts(&e))
            .collect();
        return format!("[{}]", parts.join(", "));
    }
    if let Some(inner) = cleaned.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let elem = inner.split(';').next().unwrap_or(inner);
        return format!("{}[]", wrap_for_array(&rust_to_ts(elem)));
    }
    let base = type_basename(cleaned);
    for wrapper in &["Box", "Arc", "Rc", "Cow"] {
        if base == *wrapper {
            let args = extract_generic_args(cleaned, wrapper);
            if !args.is_empty()
                && let Some(inner) = args.iter().find(|a| !a.starts_with('\''))
            {
                return rust_to_ts(inner);
            }
        }
    }
    if base == "Option"
        && let Some(inner) = extract_generic_args(cleaned, "Option").first()
    {
        return format!("{} | null", rust_to_ts(inner));
    }
    for wrapper in &["Vec", "HashSet", "BTreeSet"] {
        if base == *wrapper
            && let Some(inner) = extract_generic_args(cleaned, wrapper).first()
        {
            return format!("{}[]", wrap_for_array(&rust_to_ts(inner)));
        }
    }
    for wrapper in &["HashMap", "BTreeMap"] {
        if base == *wrapper {
            let args = extract_generic_args(cleaned, wrapper);
            let value = args.get(1).map(String::as_str).unwrap_or("unknown");
            return format!("Record<string, {}>", rust_to_ts(value));
        }
    }
    match base {
        "String" | "str" | "char" => "string".to_string(),
        "bool" => "boolean".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" => "number".to_string(),
        "()" | "void" => "void".to_string(),
        "Value" | "JsonValue" => "unknown".to_string(),
        other => other.to_string(),
    }
}

fn wrap_for_array(ts: &str) -> String {
    if ts.contains('|') || ts.contains('&') {
        format!("({ts})")
    } else {
        ts.to_string()
    }
}

fn clean_option(ty: &str) -> &str {
    ty.strip_prefix("Option<")
        .and_then(|r| r.strip_suffix('>'))
        .unwrap_or(ty)
}

fn type_basename(ty: &str) -> &str {
    let trimmed = ty.trim().trim_start_matches('&').trim_start_matches("mut ");
    let without_generics = trimmed.split('<').next().unwrap_or(trimmed);
    without_generics.split("::").last().unwrap_or(trimmed)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(manifest: &NativeManifest, output_kind: OutputKind) -> Result<String> {
    match output_kind {
        OutputKind::LogicTypeScript => render_logic_ts(manifest),
        OutputKind::ViewTypeScript => render_view_ts(manifest),
        OutputKind::BrowserGlobalJs => render_browser_global_js(manifest),
        OutputKind::RustModule => render_rust_module(manifest),
    }
}

fn render_rust_module(manifest: &NativeManifest) -> Result<String> {
    let mut out = String::new();
    out.push_str("// Generated by `cargo build`. Do not edit by hand.\n");
    out.push_str("//\n");
    out.push_str("// Registers every `#[lingxia::native]` handler discovered in this crate.\n");
    out.push_str("// `include!` this file into your crate root and call\n");
    out.push_str("// `__lingxia_native::install()` from\n");
    out.push_str("// `HostAddon::install_host_apis`.\n");
    out.push_str("#[allow(non_snake_case)]\n");
    out.push_str("mod __lingxia_native {\n");
    out.push_str("    pub fn install() {\n");
    for r in &manifest.routes {
        out.push_str(&format!(
            "        ::lingxia::host::register_host_entry(crate::{}_host());\n",
            r.fn_ident
        ));
    }
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

fn render_dto_types(manifest: &NativeManifest, used_types: &BTreeSet<String>) -> String {
    let mut out = String::new();
    for ty in used_types {
        if let Some(def) = manifest.structs.get(ty) {
            out.push_str(&render_struct_ts(ty, def));
            out.push('\n');
        } else if let Some(def) = manifest.enums.get(ty) {
            out.push_str(&render_enum_ts(ty, def));
            out.push('\n');
        } else if let Some(def) = manifest.aliases.get(ty) {
            out.push_str(&format!(
                "export type {ty} = {};\n\n",
                rust_to_ts(&def.target)
            ));
        }
    }
    out
}

fn render_fields_inline(fields: &[StructField]) -> String {
    let mut own = Vec::new();
    let mut flattened = Vec::new();
    for f in fields {
        if f.flatten {
            flattened.push(rust_to_ts(clean_option(&f.ty)));
        } else {
            let opt = if f.optional { "?" } else { "" };
            let field_ty = if f.ty.starts_with("Option<") {
                rust_to_ts(clean_option(&f.ty))
            } else {
                rust_to_ts(&f.ty)
            };
            own.push(format!(
                "{}{}: {}",
                safe_ts_property(&f.name),
                opt,
                field_ty
            ));
        }
    }
    let obj = format!("{{ {} }}", own.join("; "));
    if flattened.is_empty() {
        obj
    } else if own.is_empty() {
        flattened.join(" & ")
    } else {
        format!("({obj} & {})", flattened.join(" & "))
    }
}

fn render_struct_ts(name: &str, def: &StructDef) -> String {
    match &def.kind {
        StructDefKind::Named(fields) => {
            let mut own = Vec::new();
            let mut flattened = Vec::new();
            for f in fields {
                if f.flatten {
                    flattened.push(rust_to_ts(clean_option(&f.ty)));
                } else {
                    let opt = if f.optional { "?" } else { "" };
                    let field_ty = if f.ty.starts_with("Option<") {
                        rust_to_ts(clean_option(&f.ty))
                    } else {
                        rust_to_ts(&f.ty)
                    };
                    own.push(format!(
                        "  {}{}: {};\n",
                        safe_ts_property(&f.name),
                        opt,
                        field_ty
                    ));
                }
            }
            if flattened.is_empty() {
                format!("export interface {name} {{\n{}}}\n", own.join(""))
            } else if own.is_empty() {
                format!("export type {name} = {};\n", flattened.join(" & "))
            } else {
                format!(
                    "export type {name} = {{\n{}}} & {};\n",
                    own.join(""),
                    flattened.join(" & ")
                )
            }
        }
        StructDefKind::Tuple(elems) => {
            if elems.len() == 1 {
                format!("export type {name} = {};\n", rust_to_ts(&elems[0]))
            } else {
                let parts: Vec<String> = elems.iter().map(|e| rust_to_ts(e)).collect();
                format!("export type {name} = [{}];\n", parts.join(", "))
            }
        }
        StructDefKind::Unit => format!("export type {name} = null;\n"),
    }
}

fn render_enum_ts(name: &str, def: &EnumDef) -> String {
    if def.variants.is_empty() {
        return format!("export type {name} = never;\n");
    }
    let mut branches = Vec::with_capacity(def.variants.len());
    for v in &def.variants {
        let tag_lit = json_string(&v.wire_name);
        let branch = match &def.tagging {
            EnumTagging::External => match &v.payload {
                VariantPayload::Unit => tag_lit,
                VariantPayload::Newtype(t) => {
                    format!(
                        "{{ {}: {} }}",
                        safe_ts_property(&v.wire_name),
                        rust_to_ts(t)
                    )
                }
                VariantPayload::Tuple(ts) => {
                    let parts: Vec<String> = ts.iter().map(|t| rust_to_ts(t)).collect();
                    format!(
                        "{{ {}: [{}] }}",
                        safe_ts_property(&v.wire_name),
                        parts.join(", ")
                    )
                }
                VariantPayload::Struct(fields) => {
                    format!(
                        "{{ {}: {} }}",
                        safe_ts_property(&v.wire_name),
                        render_fields_inline(fields)
                    )
                }
            },
            EnumTagging::Internal { tag } => {
                let tag_prop = safe_ts_property(tag);
                match &v.payload {
                    VariantPayload::Unit => format!("{{ {tag_prop}: {tag_lit} }}"),
                    VariantPayload::Newtype(t) => {
                        format!("({{ {tag_prop}: {tag_lit} }} & {})", rust_to_ts(t))
                    }
                    VariantPayload::Tuple(ts) => {
                        let parts: Vec<String> = ts.iter().map(|t| rust_to_ts(t)).collect();
                        format!("({{ {tag_prop}: {tag_lit} }} & [{}])", parts.join(", "))
                    }
                    VariantPayload::Struct(fields) => {
                        let mut own = vec![format!("{tag_prop}: {tag_lit}")];
                        let mut flattened = Vec::new();
                        for f in fields {
                            if f.flatten {
                                flattened.push(rust_to_ts(clean_option(&f.ty)));
                            } else {
                                let opt = if f.optional { "?" } else { "" };
                                let field_ty = if f.ty.starts_with("Option<") {
                                    rust_to_ts(clean_option(&f.ty))
                                } else {
                                    rust_to_ts(&f.ty)
                                };
                                own.push(format!(
                                    "{}{}: {}",
                                    safe_ts_property(&f.name),
                                    opt,
                                    field_ty
                                ));
                            }
                        }
                        let obj = format!("{{ {} }}", own.join("; "));
                        if flattened.is_empty() {
                            obj
                        } else {
                            format!("({obj} & {})", flattened.join(" & "))
                        }
                    }
                }
            }
            EnumTagging::Adjacent { tag, content } => {
                let tag_prop = safe_ts_property(tag);
                let content_prop = safe_ts_property(content);
                match &v.payload {
                    VariantPayload::Unit => format!("{{ {tag_prop}: {tag_lit} }}"),
                    VariantPayload::Newtype(t) => {
                        format!(
                            "{{ {tag_prop}: {tag_lit}; {content_prop}: {} }}",
                            rust_to_ts(t)
                        )
                    }
                    VariantPayload::Tuple(ts) => {
                        let parts: Vec<String> = ts.iter().map(|t| rust_to_ts(t)).collect();
                        format!(
                            "{{ {tag_prop}: {tag_lit}; {content_prop}: [{}] }}",
                            parts.join(", ")
                        )
                    }
                    VariantPayload::Struct(fields) => {
                        format!(
                            "{{ {tag_prop}: {tag_lit}; {content_prop}: {} }}",
                            render_fields_inline(fields)
                        )
                    }
                }
            }
            EnumTagging::Untagged => match &v.payload {
                VariantPayload::Unit => "null".to_string(),
                VariantPayload::Newtype(t) => rust_to_ts(t),
                VariantPayload::Tuple(ts) => {
                    let parts: Vec<String> = ts.iter().map(|t| rust_to_ts(t)).collect();
                    format!("[{}]", parts.join(", "))
                }
                VariantPayload::Struct(fields) => render_fields_inline(fields),
            },
        };
        branches.push(branch);
    }
    format!("export type {name} = {};\n", branches.join(" | "))
}

const SHARED_TS_TYPES: &str = r#"export type NativeVoid = void;

export interface NativeCallOptions {
  signal?: AbortSignal;
}

export interface NativeTask<E = unknown, R = void> extends AsyncIterable<E, R, void> {
  readonly result: Promise<R>;
  cancel(): void;
}

export interface NativeChannel<In = unknown, Out = In> {
  send(payload: In): void;
  onMessage(listener: (payload: Out) => void): () => void;
  onClose(listener: (event: { code?: string; reason?: string }) => void): () => void;
  close(code?: string, reason?: string): void;
}
"#;

const LOGIC_DISPATCH_HELPER: &str = r#"interface LogicNativeBridge {
  invoke<R>(route: string, input?: unknown, pageId?: string): Promise<R>;
  invokeSync<R>(route: string, input?: unknown, pageId?: string): R;
  stream<E, R>(route: string, input?: unknown, options?: NativeCallOptions, pageId?: string): NativeTask<E, R>;
  channel<In, Out>(route: string, input?: unknown, pageId?: string): Promise<NativeChannel<In, Out>>;
}

function __dispatch(): LogicNativeBridge {
  const host = (globalThis as { __lingxiaNative?: LogicNativeBridge }).__lingxiaNative;
  if (!host) {
    throw new Error("E_UNAVAILABLE: LingXia Logic native dispatch is not installed in this runtime");
  }
  return host;
}
"#;

const VIEW_ADAPTER_HELPER: &str = r#"function __toTask<E, R>(
  handle: { onEvent(cb: (e: E) => void): () => void; onError?(cb: (e: unknown) => void): () => void; result: Promise<R>; cancel(): void },
  options?: NativeCallOptions,
): NativeTask<E, R> {
  if (options?.signal) {
    if (options.signal.aborted) handle.cancel();
    else options.signal.addEventListener("abort", () => handle.cancel(), { once: true });
  }
  let claimed = false;
  return {
    result: handle.result,
    cancel() { handle.cancel(); },
    [Symbol.asyncIterator](): AsyncIterator<E, R, void> {
      if (claimed) throw new Error("A task has one progress consumer");
      claimed = true;
      const queue: E[] = [];
      let done = false;
      let settled = false;
      let outcome: R;
      let failure: unknown;
      let failed = false;
      let wake: (() => void) | null = null;
      const notify = () => { const fn = wake; wake = null; if (fn) fn(); };
      const unsub = handle.onEvent((ev) => { queue.push(ev); notify(); });
      handle.result.then(
        (val) => { outcome = val; settled = true; unsub(); notify(); },
        (err) => { failure = err; failed = true; settled = true; unsub(); notify(); },
      );
      return {
        async next(): Promise<IteratorResult<E, R>> {
          while (true) {
            if (queue.length > 0) return { done: false, value: queue.shift() as E };
            if (settled) {
              done = true;
              if (failed) throw failure;
              return { done: true, value: outcome };
            }
            await new Promise<void>((resolve) => { wake = resolve; });
          }
        },
        async return(): Promise<IteratorResult<E, R>> {
          if (!done && !settled) handle.cancel();
          unsub();
          done = true;
          return { done: true, value: undefined as unknown as R };
        },
      };
    },
  };
}
"#;

fn render_logic_ts(manifest: &NativeManifest) -> Result<String> {
    let used_types = collect_and_validate_types(manifest)?;
    let mut out = String::new();
    out.push_str("// Generated by `cargo build`. Do not edit by hand.\n");
    out.push_str(SHARED_TS_TYPES);
    out.push('\n');
    out.push_str(LOGIC_DISPATCH_HELPER);
    out.push('\n');
    out.push_str(&render_dto_types(manifest, &used_types));

    let tree = RouteNode::build(&manifest.routes)?;
    out.push_str("export function createNative(pageId?: string) {\n  return ");
    out.push_str(&tree.render(2, RenderTarget::Logic));
    out.push_str(";\n}\n\n");
    out.push_str("export const native = createNative();\n");
    out.push_str("export type NativeClient = typeof native;\n");
    out.push_str("(globalThis as { __lingxiaCreatePageNative?: typeof createNative }).__lingxiaCreatePageNative = createNative;\n");
    out.push_str("declare module \"@lingxia/types/page\" {\n  interface PageNativeRegistry {\n    native: NativeClient;\n  }\n}\n");
    Ok(out)
}

fn render_view_ts(manifest: &NativeManifest) -> Result<String> {
    let used_types = collect_and_validate_types(manifest)?;
    let mut out = String::new();
    out.push_str("// Generated by `cargo build`. Do not edit by hand.\n");
    out.push_str("import { channel, invoke, stream } from \"@lingxia/bridge\";\n\n");
    out.push_str(SHARED_TS_TYPES);
    out.push('\n');
    out.push_str(VIEW_ADAPTER_HELPER);
    out.push('\n');
    out.push_str(&render_dto_types(manifest, &used_types));

    let tree = RouteNode::build(&manifest.routes)?;
    out.push_str("export const native = ");
    out.push_str(&tree.render(0, RenderTarget::View));
    out.push_str(";\n");
    out.push_str("export type NativeClient = typeof native;\n");
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
      var out = new Error(message);
      out.code = code;
      if ('data' in error) out.data = error.data;
      return out;
    }
    var err = new Error(error instanceof Error ? error.message : String(error || 'Unknown error'));
    err.code = 'BRIDGE_INTERNAL_ERROR';
    return err;
  }
  function call(parts) {
    return function (input) {
      return bridge().raw.call(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).catch(function (error) { return Promise.reject(nativeError(error)); });
    };
  }
  function stream(parts) {
    return function (input, options) {
      var opts = arguments.length === 1 && input && typeof input === 'object' && 'signal' in input ? input : options;
      var payload = opts === input ? undefined : (arguments.length === 0 ? undefined : input);
      var handle = bridge().raw.stream(route(parts), payload, { cap: 'host', timeoutMs: 0 });
      if (opts && opts.signal) {
        if (opts.signal.aborted) handle.cancel();
        else opts.signal.addEventListener('abort', function () { handle.cancel(); }, { once: true });
      }
      var result = handle.result.catch(function (error) { return Promise.reject(nativeError(error)); });
      var claimed = false;
      var task = {
        result: result,
        cancel: function () { handle.cancel(); }
      };
      task[Symbol.asyncIterator] = function () {
        if (claimed) throw new Error('A task has one progress consumer');
        claimed = true;
        var queue = [];
        var done = false;
        var settled = false;
        var outcome;
        var failure;
        var failed = false;
        var wake = null;
        function notify() { var fn = wake; wake = null; if (fn) fn(); }
        handle.on('data', function (event) { queue.push(event); notify(); });
        result.then(
          function (val) { outcome = val; settled = true; notify(); },
          function (err) { failure = err; failed = true; settled = true; notify(); }
        );
        return {
          next: function () {
            if (queue.length > 0) return Promise.resolve({ done: false, value: queue.shift() });
            if (settled) {
              done = true;
              return failed ? Promise.reject(failure) : Promise.resolve({ done: true, value: outcome });
            }
            return new Promise(function (resolve, reject) {
              wake = function () {
                if (queue.length > 0) resolve({ done: false, value: queue.shift() });
                else if (settled) {
                  done = true;
                  if (failed) reject(failure); else resolve({ done: true, value: outcome });
                }
              };
            });
          },
          return: function () {
            if (!done && !settled) handle.cancel();
            done = true;
            return Promise.resolve({ done: true, value: undefined });
          }
        };
      };
      return task;
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

// ---------------------------------------------------------------------------
// Route tree — nested TypeScript/JS object literal
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum RenderTarget {
    Logic,
    View,
}

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

    fn render(&self, indent: usize, target: RenderTarget) -> String {
        if let Some(route) = &self.route {
            return render_route_method(route, target);
        }
        let pad = " ".repeat(indent);
        let child_pad = " ".repeat(indent + 2);
        let mut out = String::from("{\n");
        for (name, child) in &self.children {
            out.push_str(&format!(
                "{child_pad}{}: {},\n",
                safe_ts_property(name),
                child.render(indent + 2, target)
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

fn render_route_method(route: &NativeRoute, target: RenderTarget) -> String {
    let input_ts = route.input.as_deref().map(rust_to_ts);
    let input_arg = input_ts
        .as_ref()
        .map(|ty| format!("input: {ty}"))
        .unwrap_or_default();

    match target {
        RenderTarget::Logic => match route.kind {
            RouteKind::Call => {
                let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}): Promise<{output}> => __dispatch().invoke<{output}>(\"{}\", input, pageId)",
                        route.route
                    )
                } else {
                    format!(
                        "(): Promise<{output}> => __dispatch().invoke<{output}>(\"{}\", undefined, pageId)",
                        route.route
                    )
                }
            }
            RouteKind::SyncCall => {
                let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}): {output} => __dispatch().invokeSync<{output}>(\"{}\", input, pageId)",
                        route.route
                    )
                } else {
                    format!(
                        "(): {output} => __dispatch().invokeSync<{output}>(\"{}\", undefined, pageId)",
                        route.route
                    )
                }
            }
            RouteKind::Stream => {
                let event = rust_to_ts(route.event.as_deref().unwrap_or("unknown"));
                let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}, options?: NativeCallOptions): NativeTask<{event}, {output}> => __dispatch().stream<{event}, {output}>(\"{}\", input, options, pageId)",
                        route.route
                    )
                } else {
                    format!(
                        "(options?: NativeCallOptions): NativeTask<{event}, {output}> => __dispatch().stream<{event}, {output}>(\"{}\", undefined, options, pageId)",
                        route.route
                    )
                }
            }
            RouteKind::Channel => {
                let inbound = rust_to_ts(route.channel_in.as_deref().unwrap_or("unknown"));
                let outbound = rust_to_ts(route.channel_out.as_deref().unwrap_or("unknown"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}): Promise<NativeChannel<{inbound}, {outbound}>> => __dispatch().channel<{inbound}, {outbound}>(\"{}\", input, pageId)",
                        route.route
                    )
                } else {
                    format!(
                        "(): Promise<NativeChannel<{inbound}, {outbound}>> => __dispatch().channel<{inbound}, {outbound}>(\"{}\", undefined, pageId)",
                        route.route
                    )
                }
            }
        },
        RenderTarget::View => match route.kind {
            RouteKind::Call | RouteKind::SyncCall => {
                let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}): Promise<{output}> => invoke<{output}, {}>(\"{}\", input)",
                        input_ts.unwrap(),
                        route.route
                    )
                } else {
                    format!(
                        "(): Promise<{output}> => invoke<{output}>(\"{}\")",
                        route.route
                    )
                }
            }
            RouteKind::Stream => {
                let event = rust_to_ts(route.event.as_deref().unwrap_or("unknown"));
                let output = rust_to_ts(route.output.as_deref().unwrap_or("void"));
                if route.input.is_some() {
                    format!(
                        "({input_arg}, options?: NativeCallOptions): NativeTask<{event}, {output}> => __toTask(stream<{event}, {output}, {}>(\"{}\", input), options)",
                        input_ts.unwrap(),
                        route.route
                    )
                } else {
                    format!(
                        "(options?: NativeCallOptions): NativeTask<{event}, {output}> => __toTask(stream<{event}, {output}>(\"{}\"), options)",
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
        },
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
        RouteKind::Call | RouteKind::SyncCall => format!("call([{parts}])"),
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
    for (i, ch) in name.chars().enumerate() {
        if ch == '_' {
            upper_next = true;
        } else if i == 0 {
            out.extend(ch.to_lowercase());
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn to_pascal_case(name: &str) -> String {
    let mut out = String::new();
    let mut upper_next = true;
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

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
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
        let mut manifest = NativeManifest::default();
        scan_file_into(source, "src/lib.rs", &mut manifest).unwrap();
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
        let view = render(&manifest, OutputKind::ViewTypeScript).unwrap();
        assert!(view.contains("deviceId: string"));
        assert!(view.contains("retryCount?: number"));
        assert!(view.contains("invoke<void, OpenDeviceInput>"));

        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(logic.contains("export function createNative(pageId?: string)"));
        assert!(logic.contains("__dispatch().invoke<void>(\"device.open\", input, pageId)"));
    }

    #[test]
    fn supports_enums_serde_attributes_tuples_and_maps() {
        let manifest = scan_source(
            r#"
            #[serde(rename_all = "kebab-case")]
            enum ThemeMode {
                LightMode,
                DarkMode,
                #[serde(rename = "auto")]
                System,
            }

            #[serde(tag = "kind", rename_all = "camelCase")]
            enum ExportEvent {
                Started,
                Progress { percent: u8, #[serde(default)] note: String },
            }

            #[serde(tag = "t", content = "c")]
            enum AdjEvent {
                Ping,
                Data(Vec<(String, u32)>),
            }

            #[serde(untagged)]
            enum FlexibleId {
                Num(u64),
                Text(String),
            }

            struct BaseMeta {
                version: u32,
            }

            #[serde(rename_all = "snake_case")]
            struct ExportInput {
                mode: crate::ThemeMode,
                #[serde(rename = "outPath")]
                output_path: String,
                #[serde(skip)]
                internal_only: u64,
                #[serde(skip_serializing_if = "Option::is_none")]
                tags: Option<std::collections::HashMap<String, FlexibleId>>,
                #[serde(flatten)]
                meta: BaseMeta,
            }

            #[lingxia::native("editor.export", stream)]
            pub async fn export(
                input: ExportInput,
                ctx: StreamContext<ExportEvent, AdjEvent>,
                _state: PageState<EditorState>,
            ) -> HostResult<()> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(
            logic.contains(r#"export type ThemeMode = "light-mode" | "dark-mode" | "auto";"#),
            "{logic}"
        );
        assert!(logic.contains(r#"{ kind: "started" }"#), "{logic}");
        assert!(
            logic.contains(r#"{ kind: "progress"; percent: number; note?: string }"#),
            "{logic}"
        );
        assert!(
            logic.contains(r#"{ t: "Ping" } | { t: "Data"; c: [string, number][] }"#),
            "{logic}"
        );
        assert!(
            logic.contains("export type FlexibleId = number | string;"),
            "{logic}"
        );
        assert!(logic.contains("outPath: string;"), "{logic}");
        assert!(!logic.contains("internal_only"), "{logic}");
        assert!(
            logic.contains("tags?: Record<string, FlexibleId>;"),
            "{logic}"
        );
        assert!(logic.contains("& BaseMeta;"), "{logic}");
        assert!(
            logic.contains("NativeTask<ExportEvent, AdjEvent>"),
            "{logic}"
        );
    }

    #[test]
    fn rejects_unresolved_dto_types_with_location() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("files.stat")]
            pub async fn stat() -> HostResult<MissingDto> { todo!() }
        "#,
        );
        let err = render(&manifest, OutputKind::LogicTypeScript)
            .unwrap_err()
            .to_string();
        assert!(err.contains("MissingDto"), "{err}");
        assert!(err.contains("src/lib.rs:"), "{err}");
        assert!(err.contains("files.stat"), "{err}");
    }

    #[test]
    fn sync_route_emits_sync_method_in_logic_and_async_in_view() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("math.add", sync)]
            pub fn add(input: (i32, i32)) -> HostResult<i32> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(logic.contains("(input: [number, number]): number => __dispatch().invokeSync<number>(\"math.add\", input, pageId)"), "{logic}");

        let view = render(&manifest, OutputKind::ViewTypeScript).unwrap();
        assert!(view.contains("(input: [number, number]): Promise<number> => invoke<number, [number, number]>(\"math.add\", input)"), "{view}");
    }

    #[test]
    fn host_extractors_are_never_treated_as_json_input() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("notes.scoped")]
            pub fn scoped(input: String, context: lingxia::host::HostInvocationContext, state: PageState<NotesState>) -> HostResult<()> { todo!() }

            #[lingxia::native("notes.ping")]
            pub fn ping(context: HostInvocationContext, state: PageState<NotesState>) -> HostResult<u32> { todo!() }
        "#,
        );
        let input = |name: &str| {
            manifest
                .routes
                .iter()
                .find(|route| route.route == name)
                .unwrap()
                .input
                .clone()
        };
        assert_eq!(input("notes.scoped").as_deref(), Some("String"));
        assert_eq!(input("notes.ping"), None);
    }

    #[test]
    fn generated_browser_js_exposes_unified_async_iterable_task() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("downloads.watch", stream)]
            pub async fn watch(ctx: StreamContext<String, ()>) -> HostResult<()> { todo!() }
        "#,
        );
        let generated = render(&manifest, OutputKind::BrowserGlobalJs).unwrap();
        assert!(generated.contains("global.native"));
        assert!(generated.contains("Symbol.asyncIterator"));
        assert!(generated.contains("stream([\"downloads\", \"watch\"])"));
    }

    #[test]
    fn rust_module_emits_install_under_lingxia_namespace() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("device.open")]
            pub fn open_device(input: String) -> HostResult<()> { todo!() }

            #[lingxia::native("device.close")]
            pub fn close_device() -> HostResult<()> { todo!() }
        "#,
        );
        let generated = render(&manifest, OutputKind::RustModule).unwrap();
        assert!(generated.contains("mod __lingxia_native"));
        assert!(
            generated.contains("::lingxia::host::register_host_entry(crate::open_device_host())")
        );
        assert!(
            generated.contains("::lingxia::host::register_host_entry(crate::close_device_host())")
        );
    }
}
