//! Code generation for `#[lingxia::native]` host handlers.
//!
//! Scans Rust source files for `#[lingxia::native]` handlers, each file's
//! `route_defaults!`, and their DTO types (`struct`, `enum`, `type`). The lingxia CLI
//! writes the clients an lxapp imports; the crate's build script writes the Rust
//! registry, which supplies each route's name and audience:
//!
//! - `native.logic.ts` — Logic client (`sync`, `NativeTask`, `NativeChannel`, `createNative(pageId)`)
//! - `native.view.ts` — WebView View client over `@lingxia/bridge`, same shapes
//! - `native.js` — the View client as a browser global, for plain HTML pages
//! - `.rs` — Rust registry (`mod __lingxia_native { pub fn install() }`)

mod external;
pub mod native_error;
pub mod syntax;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Fields, ItemEnum, ItemFn, ItemStruct, Lit, ReturnType, Token};
use syntax::{
    RouteAttr, RouteAudience, RouteDefaults, Shape, classify, parse_route_attr,
    parse_route_defaults, resolve_route_name, watch_event,
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// The Logic client's file name; the WebView clients sit beside it.
pub const LOGIC_CLIENT: &str = "native.logic.ts";

/// The WebView client an lxapp's pages import, if they call natives from the WebView.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewClient {
    TypeScript,
    BrowserGlobal,
}

impl ViewClient {
    const ALL: [ViewClient; 2] = [ViewClient::TypeScript, ViewClient::BrowserGlobal];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::TypeScript => "native.view.ts",
            Self::BrowserGlobal => "native.js",
        }
    }

    fn kind(self) -> OutputKind {
        match self {
            Self::TypeScript => OutputKind::ViewTypeScript,
            Self::BrowserGlobal => OutputKind::BrowserGlobalJs,
        }
    }
}

/// The session class the host opens an lxapp as, which decides the routes it may call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallerClass {
    /// The host's home lxapp.
    ControlApp,
    /// A host-bundled system lxapp the native host opens as a surface.
    ControlSurface,
    /// Any other lxapp.
    StandardApp,
}

impl CallerClass {
    fn admits(self, audience: RouteAudience) -> bool {
        match audience {
            RouteAudience::AppSessionOnly => true,
            RouteAudience::ControlAppOnly => self == Self::ControlApp,
            RouteAudience::ControlSurfaceOnly => self == Self::ControlSurface,
            RouteAudience::ControlAppOrSurface => self != Self::StandardApp,
        }
    }
}

/// Writes one set of clients for every route under `sources` (each a crate's `src/`; the
/// first is the lxapp's own crate) that `caller` may call into `dir`: the Logic client, and
/// `view` if given. Stale clients, or all of them when there are no routes, are removed. A
/// route name or DTO name two crates both define fails with both places. Returns warnings
/// for the caller to print.
pub fn generate_clients(
    sources: &[PathBuf],
    dir: &Path,
    view: Option<ViewClient>,
    caller: CallerClass,
) -> Result<Vec<String>> {
    let mut manifest = NativeManifest::default();
    for (index, src) in sources.iter().enumerate() {
        let mut crate_manifest = scan_dir(src)?;
        // The lxapp's own crate keeps bare file names; another crate's are prefixed by its directory.
        if let Some(name) = src.parent().and_then(Path::file_name).filter(|_| index > 0) {
            relabel(
                &mut crate_manifest,
                &format!("{}/src/", name.to_string_lossy()),
            );
        }
        external::resolve(&mut crate_manifest, src)?;
        merge_crate(&mut manifest, crate_manifest)?;
    }
    manifest
        .routes
        .retain(|route| caller.admits(route.audience));
    manifest.routes.sort_by(|a, b| a.route.cmp(&b.route));
    let mut wanted = vec![(OutputKind::LogicTypeScript, LOGIC_CLIENT)];
    for client in ViewClient::ALL {
        if view == Some(client) {
            wanted.push((client.kind(), client.file_name()));
        } else {
            remove_if_present(&dir.join(client.file_name()))?;
        }
    }
    for (kind, name) in wanted {
        let out = dir.join(name);
        if manifest.routes.is_empty() {
            remove_if_present(&out)?;
        } else {
            write_if_changed(&out, &render(&manifest, kind)?)?;
        }
    }
    wire_warnings(&manifest)
}

/// Adds one crate's scan to the clients' manifest. Its routes carry their modules' `#[cfg]`s,
/// since module paths of different crates are not comparable.
fn merge_crate(into: &mut NativeManifest, from: NativeManifest) -> Result<()> {
    for mut route in gated_routes(&from)? {
        route.module.clear();
        if let Some(first) = into.routes.iter().find(|r| r.route == route.route) {
            bail!(
                "native route `{}` is declared twice, at {} and {}",
                route.route,
                first.loc.display(),
                route.loc.display()
            );
        }
        into.routes.push(route);
    }
    for (name, def) in from.structs {
        insert_def(into, &name, Def::Struct(def))?;
    }
    for (name, def) in from.enums {
        insert_def(into, &name, Def::Enum(def))?;
    }
    for (name, def) in from.aliases {
        insert_def(into, &name, Def::Alias(def))?;
    }
    for (name, locs) in from.conflicts {
        into.conflicts.entry(name).or_default().extend(locs);
    }
    into.native_errors.extend(from.native_errors);
    for (name, path) in from.uses {
        into.uses.entry(name).or_insert(path);
    }
    Ok(())
}

/// Prefixes the file of every place a crate's own scan names.
fn relabel(manifest: &mut NativeManifest, label: &str) {
    let relabel = |loc: &mut SourceLoc| loc.file = format!("{label}{}", loc.file);
    manifest
        .routes
        .iter_mut()
        .for_each(|route| relabel(&mut route.loc));
    manifest
        .structs
        .values_mut()
        .for_each(|def| relabel(&mut def.loc));
    manifest
        .enums
        .values_mut()
        .for_each(|def| relabel(&mut def.loc));
    manifest
        .aliases
        .values_mut()
        .for_each(|def| relabel(&mut def.loc));
    manifest.conflicts.values_mut().flatten().for_each(relabel);
}

/// Whether the crate whose root is `lib_rs` declares `lingxia::routes!()`, so a host can
/// list it in `host!(…, routes = [..])`.
pub fn declares_routes(lib_rs: &Path) -> Result<bool> {
    let source =
        fs::read_to_string(lib_rs).with_context(|| format!("read {}", lib_rs.display()))?;
    let ast = syn::parse_file(&source).with_context(|| format!("parse {}", lib_rs.display()))?;
    Ok(ast.items.iter().any(|item| {
        matches!(item, syn::Item::Macro(item) if macro_named(&item.mac.path, "routes", &["lingxia", "lingxia_native_macros"]))
    }))
}

/// The crates a host's `lingxia::host!(…, routes = [..])` lists, by lib name; `None` when
/// `lib_rs` has no `host!`.
pub fn host_route_crates(lib_rs: &Path) -> Result<Option<Vec<String>>> {
    let source =
        fs::read_to_string(lib_rs).with_context(|| format!("read {}", lib_rs.display()))?;
    let ast = syn::parse_file(&source).with_context(|| format!("parse {}", lib_rs.display()))?;
    let Some(host) = ast.items.iter().find_map(|item| match item {
        syn::Item::Macro(item) if macro_named(&item.mac.path, "host", &["lingxia"]) => {
            Some(&item.mac)
        }
        _ => None,
    }) else {
        return Ok(None);
    };
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = host
        .parse_body_with(parser)
        .with_context(|| format!("parse the host! in {}", lib_rs.display()))?;
    let mut crates = Vec::new();
    for arg in &args {
        let Expr::Assign(assign) = arg else { continue };
        let (Expr::Path(key), Expr::Array(list)) = (assign.left.as_ref(), assign.right.as_ref())
        else {
            continue;
        };
        if key.path.is_ident("routes") {
            crates.extend(list.elems.iter().filter_map(|elem| match elem {
                Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
                _ => None,
            }));
        }
    }
    Ok(Some(crates))
}

/// `name!`, bare or under one of `prefixes`.
fn macro_named(path: &syn::Path, name: &str, prefixes: &[&str]) -> bool {
    let mut segments = path.segments.iter().rev();
    segments.next().is_some_and(|last| last.ident == name)
        && match (segments.next(), segments.next()) {
            (None, _) => true,
            (Some(prefix), None) => prefixes.iter().any(|p| prefix.ident == p),
            _ => false,
        }
}

/// A route crate's whole build script: writes the registry its crate root `include!`s
/// (`lingxia::host!` does that for a host crate).
pub fn build() {
    println!("cargo:rerun-if-changed=src");
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("run from a build script");
    let out_dir = std::env::var_os("OUT_DIR").expect("run from a build script");
    let src = Path::new(&manifest_dir).join("src");
    let out = Path::new(&out_dir).join("lingxia_native_handlers.rs");
    if let Err(err) = generate_rust_registry(&src, &out) {
        panic!("LingXia native routes: {err:#}");
    }
    // Apple apps link the staticlib with the Swift host; the macOS cdylib `cargo test` also
    // builds is never loaded, so only it may leave the host's symbols unresolved.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-cdylib=-Wl,-undefined,dynamic_lookup");
    }
}

/// Emit the Rust auto-register module that wraps every discovered handler's
/// `register_host_entry(...)` call inside `mod __lingxia_native { pub fn install() }`.
/// `out` must end in `.rs`.
pub fn generate_rust_registry(rust_dir: &Path, out: &Path) -> Result<()> {
    let manifest = scan_dir(rust_dir)?;
    write_if_changed(out, &render(&manifest, OutputKind::RustModule)?)
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("Failed to remove {}", path.display()))
        }
        _ => Ok(()),
    }
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

fn scan_dir(rust_dir: &Path) -> Result<NativeManifest> {
    if !rust_dir.exists() {
        bail!(
            "Native Rust API directory not found: {}",
            rust_dir.display()
        );
    }
    scan(rust_dir)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    LogicTypeScript,
    ViewTypeScript,
    BrowserGlobalJs,
    RustModule,
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteKind {
    Call,
    SyncCall,
    Stream,
    Watch,
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
    audience: RouteAudience,
    kind: RouteKind,
    input: Option<String>,
    output: Option<String>,
    /// The `E` of a `Result<T, E>` return.
    error: Option<String>,
    event: Option<String>,
    channel_in: Option<String>,
    channel_out: Option<String>,
    fn_ident: String,
    /// Module path from the crate root, and the `#[cfg]`s on the fn itself.
    module: Vec<String>,
    cfgs: Vec<String>,
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
    /// Snake_case on the wire only because no `rename_all` was written.
    unasked_snake: bool,
    ty: String,
    /// `#[serde(default)]`: may be missing on input.
    default: bool,
    /// `skip_serializing_if`: may be missing on output.
    skip_if: bool,
    flatten: bool,
}

/// Which way a DTO crosses the bridge; a field's optionality depends on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Dir {
    input: bool,
    output: bool,
}

impl Dir {
    const IN: Dir = Dir {
        input: true,
        output: false,
    };
    const OUT: Dir = Dir {
        input: false,
        output: true,
    };

    fn union(self, other: Dir) -> Dir {
        Dir {
            input: self.input || other.input,
            output: self.output || other.output,
        }
    }
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
    /// Every `mod` item, by its path from the crate root.
    modules: BTreeMap<Vec<String>, ModDecl>,
    structs: BTreeMap<String, StructDef>,
    enums: BTreeMap<String, EnumDef>,
    aliases: BTreeMap<String, TypeAliasDef>,
    /// What the native crate's `use` items import, by the name they bring in.
    uses: BTreeMap<String, external::UsePath>,
    /// DTO names defined twice with different shapes: an error once a route reaches one.
    conflicts: BTreeMap<String, Vec<SourceLoc>>,
    /// `#[derive(NativeError)]` enums; each becomes a union in the clients.
    native_errors: BTreeSet<String>,
}

impl NativeManifest {
    fn defines(&self, name: &str) -> bool {
        self.structs.contains_key(name)
            || self.enums.contains_key(name)
            || self.aliases.contains_key(name)
    }
}

enum Def {
    Struct(StructDef),
    Enum(EnumDef),
    Alias(TypeAliasDef),
}

impl Def {
    fn loc(&self) -> &SourceLoc {
        match self {
            Def::Struct(def) => &def.loc,
            Def::Enum(def) => &def.loc,
            Def::Alias(def) => &def.loc,
        }
    }

    /// What a client sees of it; two definitions with one shape are the same DTO.
    fn shape(&self) -> String {
        match self {
            Def::Struct(def) => format!("struct {:?}", def.kind),
            Def::Enum(def) => format!("enum {:?} {:?}", def.tagging, def.variants),
            Def::Alias(def) => format!("type {}", def.target),
        }
    }
}

/// DTOs are keyed by bare name, so a second definition of a name must have the same shape.
fn insert_def(manifest: &mut NativeManifest, name: &str, def: Def) -> Result<()> {
    let existing = if let Some(old) = manifest.structs.get(name) {
        Some(Def::Struct(old.clone()))
    } else if let Some(old) = manifest.enums.get(name) {
        Some(Def::Enum(old.clone()))
    } else {
        manifest.aliases.get(name).cloned().map(Def::Alias)
    };
    if let Some(old) = existing {
        if old.shape() != def.shape() {
            let locs = manifest
                .conflicts
                .entry(name.to_string())
                .or_insert_with(|| vec![old.loc().clone()]);
            locs.push(def.loc().clone());
        }
        return Ok(());
    }
    match def {
        Def::Struct(def) => manifest.structs.insert(name.to_string(), def).map(|_| ()),
        Def::Enum(def) => manifest.enums.insert(name.to_string(), def).map(|_| ()),
        Def::Alias(def) => manifest.aliases.insert(name.to_string(), def).map(|_| ()),
    };
    Ok(())
}

impl StructDef {
    fn refs(&self, out: &mut BTreeSet<String>) {
        match &self.kind {
            StructDefKind::Named(fields) => {
                for f in fields {
                    collect_refs_from_type(&f.ty, out);
                }
            }
            StructDefKind::Tuple(elems) => {
                for e in elems {
                    collect_refs_from_type(e, out);
                }
            }
            StructDefKind::Unit => {}
        }
    }
}

impl EnumDef {
    fn refs(&self, out: &mut BTreeSet<String>) {
        for v in &self.variants {
            match &v.payload {
                VariantPayload::Unit => {}
                VariantPayload::Newtype(t) => collect_refs_from_type(t, out),
                VariantPayload::Tuple(ts) => {
                    for t in ts {
                        collect_refs_from_type(t, out);
                    }
                }
                VariantPayload::Struct(fields) => {
                    for f in fields {
                        collect_refs_from_type(&f.ty, out);
                    }
                }
            }
        }
    }
}

/// Every type name a route or a DTO of the native crate names.
/// The type names route signatures use directly.
fn route_refs(manifest: &NativeManifest) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for r in &manifest.routes {
        for ty in [&r.input, &r.output, &r.event, &r.channel_in, &r.channel_out]
            .into_iter()
            .flatten()
        {
            collect_refs_from_type(ty, &mut out);
        }
    }
    out.extend(manifest.native_errors.iter().cloned());
    out
}

/// The type names the native crate's own definition of `name` uses.
fn def_refs(manifest: &NativeManifest, name: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Some(def) = manifest.structs.get(name) {
        def.refs(&mut out);
    }
    if let Some(def) = manifest.enums.get(name) {
        def.refs(&mut out);
    }
    if let Some(def) = manifest.aliases.get(name) {
        collect_refs_from_type(&def.target, &mut out);
    }
    out
}

#[derive(Debug, Clone)]
struct ModDecl {
    vis: ModVis,
    cfgs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModVis {
    Private,
    Super,
    Crate,
}

impl ModVis {
    fn of(vis: &syn::Visibility) -> Self {
        match vis {
            syn::Visibility::Public(_) => Self::Crate,
            syn::Visibility::Restricted(r) if r.path.is_ident("crate") => Self::Crate,
            syn::Visibility::Restricted(r) if r.path.is_ident("super") => Self::Super,
            _ => Self::Private,
        }
    }
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
        // `main.rs` and `bin/` are other crates; their routes are not this library's.
        if file_label == "main.rs" || file_label.starts_with("bin/") {
            continue;
        }
        scan_file_into(&source, &file_label, &mut manifest)?;
    }

    let mut seen: BTreeMap<&str, &SourceLoc> = BTreeMap::new();
    for r in &manifest.routes {
        if let Some(first) = seen.insert(&r.route, &r.loc) {
            bail!(
                "native route `{}` is declared twice, at {} and {}",
                r.route,
                first.display(),
                r.loc.display()
            );
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
    scan_items(
        &ast.items,
        source,
        file_label,
        &file_module(file_label),
        true,
        manifest,
    )
}

/// The module a file is by Rust's layout rules: `lib.rs` is the root, `a/mod.rs` and `a.rs` are `a`.
fn file_module(file_label: &str) -> Vec<String> {
    let stem = file_label.strip_suffix(".rs").unwrap_or(file_label);
    let mut parts: Vec<String> = stem.split('/').map(str::to_string).collect();
    if parts.last().is_some_and(|last| last == "mod") || (parts.len() == 1 && parts[0] == "lib") {
        parts.pop();
    }
    parts
}

/// `#[cfg(test)]`: never in a client, and free to reuse a DTO name.
fn is_test_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .meta
                .require_list()
                .is_ok_and(|list| list.tokens.to_string() == "test")
    })
}

fn item_attrs(item: &syn::Item) -> &[Attribute] {
    match item {
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        _ => &[],
    }
}

/// `file_root`: these are a file's top-level items, where `route_defaults!` may stand and
/// covers the items after it; an inline `mod` starts without defaults.
fn scan_items(
    items: &[syn::Item],
    source: &str,
    file_label: &str,
    module: &[String],
    file_root: bool,
    manifest: &mut NativeManifest,
) -> Result<()> {
    let mut defaults: Option<(RouteDefaults, usize)> = None;
    for item in items {
        if is_test_only(item_attrs(item)) {
            continue;
        }
        match item {
            syn::Item::Macro(item_macro) if is_route_defaults(&item_macro.mac.path) => {
                let line = source
                    .lines()
                    .enumerate()
                    .filter(|(_, line)| line.contains("route_defaults!"))
                    .nth(usize::from(defaults.is_some()))
                    .map_or(0, |(index, _)| index + 1);
                let located = |message: String| anyhow!("{file_label}:{line}: {message}");
                if !file_root {
                    return Err(located(
                        "route_defaults! goes at the top level of a file; it does not apply inside an inline `mod`".into(),
                    ));
                }
                if let Some((_, first)) = defaults {
                    return Err(located(format!(
                        "a second route_defaults! in this file; the first is at line {first}"
                    )));
                }
                let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
                let parsed = item_macro
                    .mac
                    .parse_body_with(parser)
                    .and_then(|args| parse_route_defaults(&args))
                    .map_err(|err| located(err.to_string()))?;
                defaults = Some((parsed, line));
            }
            syn::Item::Fn(item_fn) => {
                if let Some((attr, framework)) = parse_attr(&item_fn.attrs) {
                    let ident = item_fn.sig.ident.to_string();
                    let loc = SourceLoc {
                        file: file_label.to_string(),
                        line: find_item_line(source, "fn", &ident),
                    };
                    let defaults = defaults.as_ref().map(|(defaults, _)| defaults);
                    let mut route = extract_route_info(&attr, framework, defaults, item_fn, loc)?;
                    route.module = module.to_vec();
                    manifest.routes.push(route);
                }
            }
            syn::Item::Struct(item_struct) => {
                let ident = item_struct.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "struct", &ident),
                };
                if let Some(def) = extract_struct_def(item_struct, loc) {
                    insert_def(manifest, &ident, Def::Struct(def))?;
                }
            }
            syn::Item::Enum(item_enum) => {
                let ident = item_enum.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "enum", &ident),
                };
                if native_error::derives_native_error(&item_enum.attrs) {
                    native_error::read(item_enum)
                        .map_err(|err| anyhow!("{}: `{ident}`: {err}", loc.display()))?;
                    manifest.native_errors.insert(ident.clone());
                }
                if let Some(def) = extract_enum_def(item_enum, loc) {
                    insert_def(manifest, &ident, Def::Enum(def))?;
                }
            }
            syn::Item::Type(item_type) => {
                let ident = item_type.ident.to_string();
                let loc = SourceLoc {
                    file: file_label.to_string(),
                    line: find_item_line(source, "type", &ident),
                };
                let target = type_string(&item_type.ty).replace(' ', "");
                insert_def(manifest, &ident, Def::Alias(TypeAliasDef { target, loc }))?;
            }
            syn::Item::Use(item_use) => {
                let mut imports = Vec::new();
                external::flatten_use(&item_use.tree, &mut Vec::new(), &mut imports);
                for (name, path) in imports {
                    if !path.glob {
                        manifest.uses.entry(name).or_insert(path);
                    }
                }
            }
            syn::Item::Mod(item_mod) => {
                let mut inner = module.to_vec();
                inner.push(item_mod.ident.unraw().to_string());
                if let Some((_, sub_items)) = &item_mod.content {
                    scan_items(sub_items, source, file_label, &inner, false, manifest)?;
                }
                let decl = ModDecl {
                    vis: ModVis::of(&item_mod.vis),
                    cfgs: cfgs(&item_mod.attrs),
                };
                manifest.modules.insert(inner, decl);
            }
            _ => {}
        }
    }
    Ok(())
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

/// The route attribute, and whether it is `framework_native`. An attribute that does not
/// parse is skipped: the macro reports it at its span.
fn parse_attr(attrs: &[Attribute]) -> Option<(RouteAttr, bool)> {
    let attr = attrs.iter().find(|attr| is_native_attr(attr))?;
    let framework = attr
        .path()
        .segments
        .last()
        .is_some_and(|last| last.ident == "framework_native");
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = match &attr.meta {
        syn::Meta::Path(_) => Punctuated::new(),
        meta => meta.require_list().ok()?.parse_args_with(parser).ok()?,
    };
    parse_route_attr(&args, "native")
        .ok()
        .map(|attr| (attr, framework))
}

/// `route_defaults!`, bare or as `lingxia::route_defaults!`.
fn is_route_defaults(path: &syn::Path) -> bool {
    macro_named(path, "route_defaults", &["lingxia"])
}

/// `native` / `framework_native`, bare or as `lingxia::…`.
fn is_native_attr(attr: &Attribute) -> bool {
    let path = attr.path();
    let mut segments = path.segments.iter().rev();
    let Some(last) = segments.next() else {
        return false;
    };
    let named = last.ident == "native" || last.ident == "framework_native";
    named
        && match (segments.next(), segments.next()) {
            (None, _) => true,
            (Some(prefix), None) => prefix.ident == "lingxia",
            _ => false,
        }
}

fn extract_route_info(
    attr: &RouteAttr,
    framework: bool,
    defaults: Option<&RouteDefaults>,
    item_fn: &ItemFn,
    loc: SourceLoc,
) -> Result<NativeRoute> {
    let shape = attr.shape;
    let kind = match shape {
        Shape::Call => RouteKind::Call,
        Shape::Sync => RouteKind::SyncCall,
        Shape::Stream => RouteKind::Stream,
        Shape::Watch => RouteKind::Watch,
        Shape::Channel => RouteKind::Channel,
    };
    let fn_ident = item_fn.sig.ident.unraw().to_string();
    let route = resolve_route_name(
        attr.name.as_ref().map(|name| name.value()).as_deref(),
        &fn_ident,
        defaults.and_then(|defaults| defaults.prefix.as_deref()),
    )
    .map_err(|err| anyhow!("{}: {err}", loc.display()))?;
    let route = route.as_str();
    let audience = match attr
        .audience
        .or(defaults.and_then(|defaults| defaults.audience))
    {
        Some(audience) => audience,
        None if framework => bail!(
            "{}: framework route `{route}` needs `audience = \"…\"`, on the route or in route_defaults!",
            loc.display()
        ),
        None => RouteAudience::AppSessionOnly,
    };
    let located = |err: syn::Error| anyhow!("{}:{} `{route}`: {err}", loc.file, loc.line);
    let params = classify(&item_fn.sig, shape).map_err(located)?;
    let text = |ty: Option<&syn::Type>| ty.map(|ty| type_string(ty).replace(' ', ""));
    let input = text(params.payload());
    let (event, mut output) = if kind == RouteKind::Watch {
        let event = watch_event(&item_fn.sig).map_err(located)?;
        (text(Some(&event)), Some("void".to_string()))
    } else {
        (params.progress().and_then(text), None)
    };
    let (channel_in, channel_out) = params
        .channel()
        .map(|(inbound, outbound)| (text(inbound), text(outbound)))
        .unwrap_or_default();

    let error = match &item_fn.sig.output {
        ReturnType::Type(_, ty) => unwrap_result_error(&type_string(ty).replace(' ', "")),
        ReturnType::Default => None,
    };
    if output.is_none() {
        output = match &item_fn.sig.output {
            ReturnType::Type(_, ty) => {
                let s = type_string(ty).replace(' ', "");
                unwrap_result(&s)
            }
            ReturnType::Default => Some("void".to_string()),
        };
    }

    Ok(NativeRoute {
        route: route.to_string(),
        audience,
        kind,
        input,
        output,
        error,
        event,
        channel_in,
        channel_out,
        fn_ident: item_fn.sig.ident.to_string(),
        module: Vec::new(),
        cfgs: cfgs(&item_fn.attrs),
        loc,
    })
}

fn cfgs(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .map(|attr| quote::quote!(#attr).to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Serde attribute parsing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SerdeContainerAttrs {
    rename_all: Option<RenameRule>,
    rename_all_fields: Option<RenameRule>,
    tag: Option<String>,
    content: Option<String>,
    untagged: bool,
}

#[derive(Default)]
struct SerdeMemberAttrs {
    rename: Option<String>,
    rename_all: Option<RenameRule>,
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
            } else if meta.path.is_ident("rename_all_fields") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.rename_all_fields = Some(RenameRule::parse(&s.value()));
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
            } else if meta.path.is_ident("rename_all") {
                if let Ok(value) = meta.value()
                    && let Ok(Lit::Str(s)) = value.parse::<Lit>()
                {
                    out.rename_all = Some(RenameRule::parse(&s.value()));
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

/// Wire names as serde derives them: `rule` when written, else the Rust name.
fn extract_named_fields(fields: &syn::FieldsNamed, rule: Option<RenameRule>) -> Vec<StructField> {
    fields
        .named
        .iter()
        .filter_map(|field| {
            let serde = parse_serde_member_attrs(&field.attrs);
            if serde.skip {
                return None;
            }
            let raw_name = field.ident.as_ref()?.unraw().to_string();
            let unasked_snake = serde.rename.is_none() && rule.is_none() && raw_name.contains('_');
            let name = serde
                .rename
                .unwrap_or_else(|| rule.unwrap_or(RenameRule::None).apply(&raw_name));
            let ty_str = type_string(&field.ty).replace(' ', "");
            Some(StructField {
                name,
                unasked_snake,
                ty: ty_str,
                default: serde.default,
                skip_if: serde.skip_serializing_if,
                flatten: serde.flatten,
            })
        })
        .collect()
}

fn extract_struct_def(item: &ItemStruct, loc: SourceLoc) -> Option<StructDef> {
    let container = parse_serde_container_attrs(&item.attrs);
    let kind = match &item.fields {
        Fields::Named(named) => {
            StructDefKind::Named(extract_named_fields(named, container.rename_all))
        }
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
        let raw_name = variant.ident.unraw().to_string();
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
                let field_rule = member.rename_all.or(container.rename_all_fields);
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
    for wrapper in &["Result", "std::result::Result", "lingxia::Result"] {
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

fn unwrap_result_error(ty: &str) -> Option<String> {
    ["Result", "std::result::Result"]
        .iter()
        .map(|wrapper| extract_generic_args(ty, wrapper))
        .find(|args| !args.is_empty())
        .and_then(|args| args.get(1).map(|error| error.trim().to_string()))
}

/// Types from outside the crate that serde writes as a string.
fn is_outside_string_type(name: &str) -> bool {
    matches!(
        name,
        "PathBuf"
            | "Path"
            | "OsString"
            | "Uuid"
            | "Url"
            | "DateTime"
            | "NaiveDate"
            | "NaiveDateTime"
            | "NaiveTime"
            | "IpAddr"
            | "Ipv4Addr"
            | "Ipv6Addr"
            | "SocketAddr"
    )
}

fn is_builtin_rust_type(name: &str) -> bool {
    is_outside_string_type(name)
        || matches!(
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
/// Every DTO the routes reach, with the directions it travels in.
fn collect_and_validate_types(manifest: &NativeManifest) -> Result<BTreeMap<String, Dir>> {
    let mut pending: Vec<(String, Dir, String, SourceLoc)> = Vec::new();
    for r in &manifest.routes {
        for (ty, dir) in [
            (r.input.as_deref(), Dir::IN),
            (r.output.as_deref(), Dir::OUT),
            (r.event.as_deref(), Dir::OUT),
            (r.channel_in.as_deref(), Dir::IN),
            (r.channel_out.as_deref(), Dir::OUT),
        ] {
            let Some(ty) = ty else { continue };
            let mut refs = BTreeSet::new();
            collect_refs_from_type(ty, &mut refs);
            for name in refs {
                pending.push((name, dir, format!("route `{}`", r.route), r.loc.clone()));
            }
        }
    }
    for name in &manifest.native_errors {
        if let Some(def) = manifest.enums.get(name) {
            let owner = format!("native error `{name}`");
            pending.push((name.clone(), Dir::OUT, owner, def.loc.clone()));
        }
    }

    let mut resolved: BTreeMap<String, Dir> = BTreeMap::new();
    while let Some((name, dir, owner, loc)) = pending.pop() {
        let known = resolved.get(&name).copied();
        if known.is_some_and(|known| known.union(dir) == known) {
            continue;
        }
        let dir = known.unwrap_or_default().union(dir);
        if let Some(locs) = manifest.conflicts.get(&name) {
            let locs: Vec<String> = locs.iter().map(SourceLoc::display).collect();
            bail!(
                "native DTO `{name}`, referenced by {owner}, is defined with different fields at {}; rename one",
                locs.join(" and ")
            );
        }
        if let Some(def) = manifest.structs.get(&name) {
            resolved.insert(name.clone(), dir);
            let mut child_refs = BTreeSet::new();
            match &def.kind {
                StructDefKind::Named(fields) => {
                    for f in fields {
                        let mut refs = BTreeSet::new();
                        collect_refs_from_type(&f.ty, &mut refs);
                        let owner = format!("field `{}` of `{name}`", f.name);
                        for child in refs {
                            pending.push((child, dir, owner.clone(), def.loc.clone()));
                        }
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
                pending.push((child, dir, format!("struct `{name}`"), def.loc.clone()));
            }
        } else if let Some(def) = manifest.enums.get(&name) {
            resolved.insert(name.clone(), dir);
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
                            let mut refs = BTreeSet::new();
                            collect_refs_from_type(&f.ty, &mut refs);
                            let owner = format!("field `{}` of `{name}::{}`", f.name, v.wire_name);
                            for child in refs {
                                pending.push((child, dir, owner.clone(), def.loc.clone()));
                            }
                        }
                    }
                }
            }
            for child in child_refs {
                pending.push((child, dir, format!("enum `{name}`"), def.loc.clone()));
            }
        } else if let Some(def) = manifest.aliases.get(&name) {
            resolved.insert(name.clone(), dir);
            let mut child_refs = BTreeSet::new();
            collect_refs_from_type(&def.target, &mut child_refs);
            for child in child_refs {
                pending.push((child, dir, format!("type `{name}`"), def.loc.clone()));
            }
        } else {
            bail!(
                "{}: {owner} has type `{name}`, which codegen cannot map to TypeScript; wrap it in a newtype the native crate defines (`struct {name}Value(String)` with serde), or use a mapped type: `String`, a number, `bool`, `Option`, `Vec`, `HashMap`, `Uuid`, `DateTime`, `Url`, `PathBuf`",
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
        name if is_outside_string_type(name) => "string".to_string(),
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

/// Route DTOs whose fields reach JS in snake_case only because no `rename_all` was written.
fn wire_warnings(manifest: &NativeManifest) -> Result<Vec<String>> {
    let snake = |fields: &[StructField]| {
        let names: Vec<_> = fields
            .iter()
            .filter(|f| f.unasked_snake)
            .map(|f| format!("`{}`", f.name))
            .collect();
        (!names.is_empty()).then(|| names.join(", "))
    };
    let wide = |fields: &[StructField]| {
        fields
            .iter()
            .filter(|f| is_wide_integer(&f.ty))
            .map(|f| format!("`{}`", f.name))
            .collect::<Vec<_>>()
    };
    let mut out = Vec::new();
    for name in collect_and_validate_types(manifest)?.into_keys() {
        let wide_fields = match (manifest.structs.get(&name), manifest.enums.get(&name)) {
            (Some(def), _) => match &def.kind {
                StructDefKind::Named(fields) => Some((&def.loc, wide(fields))),
                _ => None,
            },
            (None, Some(def)) => Some((
                &def.loc,
                def.variants
                    .iter()
                    .flat_map(|v| match &v.payload {
                        VariantPayload::Struct(fields) => wide(fields),
                        _ => Vec::new(),
                    })
                    .collect(),
            )),
            _ => None,
        };
        if let Some((loc, fields)) = wide_fields
            && !fields.is_empty()
        {
            out.push(format!(
                "{}: `{name}` sends {} to JS as a number, which is exact only up to 2^53; use a narrower integer or a String if values can be larger",
                loc.display(),
                fields.join(", ")
            ));
        }
        if let Some(def) = manifest.structs.get(&name)
            && let StructDefKind::Named(fields) = &def.kind
            && let Some(names) = snake(fields)
        {
            out.push(format!(
                "{}: `{name}` sends {names} to JS in snake_case; add #[serde(rename_all = \"camelCase\")]",
                def.loc.display()
            ));
        }
        if let Some(def) = manifest.enums.get(&name) {
            let names: Vec<_> = def
                .variants
                .iter()
                .filter_map(|v| match &v.payload {
                    VariantPayload::Struct(fields) => snake(fields),
                    _ => None,
                })
                .collect();
            if !names.is_empty() {
                out.push(format!(
                    "{}: `{name}` sends {} to JS in snake_case; add #[serde(rename_all_fields = \"camelCase\")]",
                    def.loc.display(),
                    names.join(", ")
                ));
            }
        }
    }
    Ok(out)
}

/// A 64-bit or wider integer, bare or in `Option`/`Vec`.
fn is_wide_integer(ty: &str) -> bool {
    let ty = ty.replace(' ', "");
    ["u64", "i64", "u128", "i128"]
        .iter()
        .any(|wide| ty == *wide || ty.ends_with(&format!("<{wide}>")))
}

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
    out.push_str("// Registers every `#[lingxia::native]` handler in this crate.\n");
    out.push_str("#[allow(non_snake_case)]\n");
    out.push_str("mod __lingxia_native {\n");
    out.push_str("    pub fn install() {\n");
    for r in &manifest.routes {
        for cfg in registry_cfgs(manifest, r)? {
            out.push_str(&format!("        {cfg}\n"));
        }
        let path: String = r.module.iter().map(|m| format!("{m}::")).collect();
        let (namespace, method) = r
            .route
            .rsplit_once('.')
            .ok_or_else(|| anyhow!("{}: route `{}` has no namespace", r.loc.display(), r.route))?;
        out.push_str(&format!(
            "        ::lingxia::host::register_host_entry(crate::{path}__lingxia_route_{}({namespace:?}, {method:?}, ::lingxia::host::RouteAudience::{}));\n",
            r.fn_ident,
            r.audience.variant()
        ));
    }
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

/// The `#[cfg]`s that gate a route: those on every `mod` above it, then its own. Fails when
/// the crate root cannot name the route's module, which would otherwise surface as a privacy
/// error inside generated code.
fn registry_cfgs(manifest: &NativeManifest, route: &NativeRoute) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for depth in 1..=route.module.len() {
        let path = &route.module[..depth];
        let name = path.join("::");
        let Some(decl) = manifest.modules.get(path) else {
            bail!(
                "{}: `{}` is in `{name}`, which no `mod` declares (a `#[path]` module is not followed); the route cannot be registered",
                route.loc.display(),
                route.route
            );
        };
        let reachable =
            depth == 1 || decl.vis == ModVis::Crate || (decl.vis == ModVis::Super && depth == 2);
        if !reachable {
            bail!(
                "{}: `{}` is in `{name}`, which the crate root cannot reach; declare it `pub(crate) mod {}`",
                route.loc.display(),
                route.route,
                path[depth - 1]
            );
        }
        out.extend(decl.cfgs.iter().cloned());
    }
    out.extend(route.cfgs.iter().cloned());
    Ok(out)
}

/// The routes with every `#[cfg]` that gates them, their modules' included.
fn gated_routes(manifest: &NativeManifest) -> Result<Vec<NativeRoute>> {
    manifest
        .routes
        .iter()
        .map(|route| {
            let mut route = route.clone();
            route.cfgs = registry_cfgs(manifest, &route)?;
            Ok(route)
        })
        .collect()
}

/// A TS doc line for a route that only some builds register.
fn cfg_doc(route: &NativeRoute, pad: &str) -> String {
    if route.cfgs.is_empty() {
        return String::new();
    }
    let conditions: Vec<&str> = route
        .cfgs
        .iter()
        .map(|cfg| {
            cfg.trim()
                .trim_start_matches("# [cfg (")
                .trim_end_matches(")]")
                .trim()
        })
        .collect();
    format!(
        "{pad}/** Only where `cfg({})`; elsewhere the call fails with `E_NOT_FOUND`. */\n",
        conditions.join(") and cfg(")
    )
}

fn render_dto_types(manifest: &NativeManifest, used_types: &BTreeMap<String, Dir>) -> String {
    let mut out = String::new();
    for (ty, &dir) in used_types {
        if let Some(def) = manifest.structs.get(ty) {
            out.push_str(&render_struct_ts(ty, def, dir));
            out.push('\n');
        } else if let Some(def) = manifest.enums.get(ty) {
            if manifest.native_errors.contains(ty) {
                out.push_str(&render_native_error_ts(ty, def));
            } else {
                out.push_str(&render_enum_ts(ty, def, dir));
            }
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

/// `name?: T | null` as serde reads it (input) or writes it (output).
fn field_ts(f: &StructField, dir: Dir) -> String {
    let option = f.ty.starts_with("Option<");
    let optional = (dir.input && (option || f.default)) || (dir.output && f.skip_if);
    let ty = if option {
        format!("{} | null", rust_to_ts(clean_option(&f.ty)))
    } else {
        rust_to_ts(&f.ty)
    };
    let opt = if optional { "?" } else { "" };
    format!("{}{opt}: {ty}", safe_ts_property(&f.name))
}

fn split_fields(fields: &[StructField], dir: Dir) -> (Vec<String>, Vec<String>) {
    let (flat, own): (Vec<_>, Vec<_>) = fields.iter().partition(|f| f.flatten);
    (
        own.into_iter().map(|f| field_ts(f, dir)).collect(),
        flat.into_iter()
            .map(|f| rust_to_ts(clean_option(&f.ty)))
            .collect(),
    )
}

fn render_fields_inline(fields: &[StructField], dir: Dir) -> String {
    let (own, flattened) = split_fields(fields, dir);
    let obj = format!("{{ {} }}", own.join("; "));
    if flattened.is_empty() {
        obj
    } else if own.is_empty() {
        flattened.join(" & ")
    } else {
        format!("({obj} & {})", flattened.join(" & "))
    }
}

fn render_struct_ts(name: &str, def: &StructDef, dir: Dir) -> String {
    match &def.kind {
        StructDefKind::Named(fields) => {
            let (own, flattened) = split_fields(fields, dir);
            let own: Vec<_> = own.into_iter().map(|f| format!("  {f};\n")).collect();
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

fn render_enum_ts(name: &str, def: &EnumDef, dir: Dir) -> String {
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
                        render_fields_inline(fields, dir)
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
                        let (fields, flattened) = split_fields(fields, dir);
                        let mut own = vec![format!("{tag_prop}: {tag_lit}")];
                        own.extend(fields);
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
                            render_fields_inline(fields, dir)
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
                VariantPayload::Struct(fields) => render_fields_inline(fields, dir),
            },
        };
        branches.push(branch);
    }
    format!("export type {name} = {};\n", branches.join(" | "))
}

/// One branch per app code; the variant's fields arrive as `data`.
fn render_native_error_ts(name: &str, def: &EnumDef) -> String {
    let mut branches: Vec<String> = def
        .variants
        .iter()
        .map(|v| {
            let data = match &v.payload {
                VariantPayload::Unit => None,
                VariantPayload::Struct(fields) if fields.is_empty() => None,
                VariantPayload::Struct(fields) => {
                    let optional = fields.iter().all(|f| f.skip_if);
                    let opt = if optional { "?" } else { "" };
                    Some(format!(
                        "data{opt}: {}",
                        render_fields_inline(fields, Dir::OUT)
                    ))
                }
                VariantPayload::Newtype(t) => Some(format!("data: {}", rust_to_ts(t))),
                VariantPayload::Tuple(_) => None,
            };
            let data = data.map(|d| format!("; {d}")).unwrap_or_default();
            format!(
                "{{ code: {}; message: string; reason?: string{data} }}",
                json_string(&v.wire_name)
            )
        })
        .collect();
    branches.push("FrameworkError".to_string());
    format!("export type {name} = {};\n", branches.join(" | "))
}

/// `isNativeError(e, code)` narrows to the errors with that code, app or framework.
fn render_native_error_guard(manifest: &NativeManifest) -> String {
    let app = if manifest.native_errors.is_empty() {
        "never".to_string()
    } else {
        manifest
            .native_errors
            .iter()
            .map(|name| format!("Exclude<{name}, FrameworkError>"))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    format!(
        r#"export type AppNativeError = {app};
export type NativeErrorCode = AppNativeError["code"] | FrameworkError["code"];
export type NativeErrorOf<C extends NativeErrorCode> = C extends FrameworkError["code"]
  ? FrameworkError & {{ code: C }}
  : Extract<AppNativeError, {{ code: C }}>;

export function isNativeError<C extends NativeErrorCode = NativeErrorCode>(
  error: unknown,
  code?: C,
): error is NativeErrorOf<C> {{
  if (!error || typeof error !== "object") return false;
  const candidate = error as {{ code?: unknown; message?: unknown }};
  if (typeof candidate.code !== "string" || typeof candidate.message !== "string") return false;
  return code === undefined || candidate.code === code;
}}
"#
    )
}

const SHARED_TS_TYPES: &str = r#"export type NativeVoid = void;

/** What a call reads of an `AbortSignal`, so a program without DOM types still checks. */
export interface NativeAbortSignal {
  readonly aborted: boolean;
  addEventListener(type: "abort", listener: () => void, options?: { once?: boolean }): void;
}

export interface NativeCallOptions {
  signal?: NativeAbortSignal;
}

export interface NativeTask<E = unknown, R = void> extends AsyncIterable<E, R, void> {
  readonly result: Promise<R>;
  cancel(): void;
}

export interface NativeError {
  code: string;
  message: string;
  reason?: string;
  data?: unknown;
}

/** A failure the framework raised: invalid input, cancel, timeout, an internal error. */
export interface FrameworkError extends NativeError {
  code: `E_${string}`;
}

export interface NativeChannel<In = unknown, Out = In> {
  send(payload: In): void;
  onMessage(listener: (payload: Out) => void): () => void;
  /** `error` is set when the channel ended in failure. */
  onClose(listener: (event: { error?: NativeError }) => void): () => void;
  close(error?: NativeError): void;
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

const WATCH_HELPER: &str = r#"const __WATCH_RETRY_MS = 1_000;
const __WATCH_RETRY_MAX_MS = 15_000;
// A reconnect cannot fix these, so they end the watch.
const __WATCH_LASTING = ["E_NOT_FOUND", "E_PERMISSION_DENIED", "E_INVALID_ARG", "E_NOT_SUPPORTED", "E_CAPABILITY_MISSING"];

function __watch<E>(
  open: (options: NativeCallOptions) => AsyncIterable<E, unknown, void>,
  onEvent: (event: E) => void,
  options?: NativeCallOptions,
): () => void {
  const stop = new AbortController();
  const outer = options?.signal;
  if (outer?.aborted) stop.abort();
  else outer?.addEventListener("abort", () => stop.abort(), { once: true });
  const { signal } = stop;
  void (async () => {
    let delay = __WATCH_RETRY_MS;
    while (!signal.aborted) {
      // Each attempt gets its own signal, so a finished one leaves no listener behind.
      const attempt = new AbortController();
      const relay = () => attempt.abort();
      signal.addEventListener("abort", relay, { once: true });
      try {
        for await (const event of open({ signal: attempt.signal })) {
          if (signal.aborted) return;
          delay = __WATCH_RETRY_MS;
          onEvent(event);
        }
      } catch (error) {
        if (signal.aborted) return;
        const code = (error as { code?: unknown } | null)?.code;
        if (typeof code === "string" && __WATCH_LASTING.includes(code)) {
          console.warn("native watch stopped", error);
          return;
        }
        console.warn("native watch failed; reconnecting", error);
      } finally {
        signal.removeEventListener("abort", relay);
      }
      if (signal.aborted) return;
      await new Promise<void>((resolve) => {
        const done = () => { clearTimeout(timer); signal.removeEventListener("abort", done); resolve(); };
        const timer = setTimeout(done, delay);
        signal.addEventListener("abort", done, { once: true });
      });
      delay = Math.min(delay * 2, __WATCH_RETRY_MAX_MS);
    }
  })();
  return () => stop.abort();
}
"#;

fn render_logic_ts(manifest: &NativeManifest) -> Result<String> {
    let used_types = collect_and_validate_types(manifest)?;
    let mut out = String::new();
    out.push_str("// Generated by the lingxia CLI from the native crate's routes. Do not edit.\n");
    out.push_str(SHARED_TS_TYPES);
    out.push('\n');
    out.push_str(LOGIC_DISPATCH_HELPER);
    out.push('\n');
    if manifest.routes.iter().any(|r| r.kind == RouteKind::Watch) {
        out.push_str(WATCH_HELPER);
        out.push('\n');
    }
    out.push_str(&render_dto_types(manifest, &used_types));
    out.push_str(&render_native_error_guard(manifest));
    out.push('\n');

    let tree = RouteNode::build(&gated_routes(manifest)?)?;
    out.push_str("export function createNative(pageId?: string) {\n  return ");
    out.push_str(&tree.render(2, RenderTarget::Logic));
    out.push_str(";\n}\n\n");
    out.push_str("export const native = createNative();\n");
    out.push_str("export type NativeClient = typeof native;\n");
    out.push_str("(globalThis as { __lingxiaCreatePageNative?: typeof createNative }).__lingxiaCreatePageNative = createNative;\n");
    out.push_str("declare module \"@lingxia/types/page\" {\n  interface PageNativeRegistry {\n    native: NativeClient;\n  }\n}\n");
    out.push_str(&render_native_mock_registry(manifest));
    Ok(out)
}

/// Each mockable route's types, so a `'native route'` handler in `mocks/index.ts` checks
/// against the route.
fn render_native_mock_registry(manifest: &NativeManifest) -> String {
    let mut out = String::from(
        "declare module \"@lingxia/types/mocks\" {\n  interface NativeMockRegistry {\n    routes: {\n",
    );
    for route in &manifest.routes {
        if route.kind == RouteKind::Channel {
            continue;
        }
        let input = route
            .input
            .as_deref()
            .map_or("undefined".to_string(), rust_to_ts);
        let result = rust_to_ts(route.output.as_deref().unwrap_or("void"));
        let event = match route.kind {
            RouteKind::Stream | RouteKind::Watch => {
                rust_to_ts(route.event.as_deref().unwrap_or("unknown"))
            }
            _ => "never".to_string(),
        };
        let error = route
            .error
            .as_deref()
            .map(|error| error.rsplit("::").next().unwrap_or(error))
            .filter(|error| manifest.native_errors.contains(*error))
            .unwrap_or("FrameworkError");
        out.push_str(&format!(
            "      {}: {{ input: {input}; result: {result}; event: {event}; error: {error} }};\n",
            json_string(&route.route)
        ));
    }
    out.push_str("    };\n  }\n}\n");
    out
}

fn render_view_ts(manifest: &NativeManifest) -> Result<String> {
    let used_types = collect_and_validate_types(manifest)?;
    let mut out = String::new();
    out.push_str("// Generated by the lingxia CLI from the native crate's routes. Do not edit.\n");
    out.push_str("import { channel, invoke, stream } from \"@lingxia/bridge\";\n\n");
    out.push_str(SHARED_TS_TYPES);
    out.push('\n');
    out.push_str(VIEW_ADAPTER_HELPER);
    out.push('\n');
    if manifest.routes.iter().any(|r| r.kind == RouteKind::Watch) {
        out.push_str(WATCH_HELPER);
        out.push('\n');
    }
    out.push_str(&render_dto_types(manifest, &used_types));
    out.push_str(&render_native_error_guard(manifest));
    out.push('\n');

    let tree = RouteNode::build(&gated_routes(manifest)?)?;
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

const NATIVE_CLIENT_JS_PREAMBLE: &str = r#"// Generated by the lingxia CLI from the native crate's routes. Do not edit.
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
      var code = typeof error.code === 'string' && error.code ? error.code : 'E_INTERNAL';
      var message = typeof error.message === 'string' && error.message ? error.message : 'Unknown error';
      var out = new Error(message);
      out.code = code;
      if (typeof error.reason === 'string') out.reason = error.reason;
      if ('data' in error) out.data = error.data;
      return out;
    }
    var err = new Error(error instanceof Error ? error.message : String(error || 'Unknown error'));
    err.code = 'E_INTERNAL';
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
  var WATCH_RETRY_MS = 1000;
  var WATCH_RETRY_MAX_MS = 15000;
  var WATCH_LASTING = ['E_NOT_FOUND', 'E_PERMISSION_DENIED', 'E_INVALID_ARG', 'E_NOT_SUPPORTED', 'E_CAPABILITY_MISSING'];
  function watch(parts) {
    var open = stream(parts);
    return function () {
      var withInput = typeof arguments[0] !== 'function';
      var input = withInput ? arguments[0] : undefined;
      var onEvent = arguments[withInput ? 1 : 0];
      var options = arguments[withInput ? 2 : 1];
      var stop = new AbortController();
      var outer = options && options.signal;
      if (outer) {
        if (outer.aborted) stop.abort();
        else outer.addEventListener('abort', function () { stop.abort(); }, { once: true });
      }
      var signal = stop.signal;
      (async function () {
        var delay = WATCH_RETRY_MS;
        while (!signal.aborted) {
          var attempt = new AbortController();
          var relay = function () { attempt.abort(); };
          signal.addEventListener('abort', relay, { once: true });
          try {
            var task = withInput ? open(input, { signal: attempt.signal }) : open({ signal: attempt.signal });
            for await (var event of task) {
              if (signal.aborted) return;
              delay = WATCH_RETRY_MS;
              onEvent(event);
            }
          } catch (error) {
            if (signal.aborted) return;
            if (error && WATCH_LASTING.indexOf(error.code) >= 0) {
              console.warn('native watch stopped', error);
              return;
            }
            console.warn('native watch failed; reconnecting', error);
          } finally {
            signal.removeEventListener('abort', relay);
          }
          if (signal.aborted) return;
          await new Promise(function (resolve) {
            var timer = setTimeout(done, delay);
            function done() { clearTimeout(timer); signal.removeEventListener('abort', done); resolve(); }
            signal.addEventListener('abort', done, { once: true });
          });
          delay = Math.min(delay * 2, WATCH_RETRY_MAX_MS);
        }
      })();
      return function () { stop.abort(); };
    };
  }
  function channel(parts) {
    return function (input) {
      return bridge().raw.channel.open(route(parts), arguments.length === 0 ? undefined : input, { cap: 'host' }).then(function (handle) {
        var messageListeners = [];
        var closeListeners = [];
        handle.on('data', function (message) { messageListeners.slice().forEach(function (listener) { listener(message); }); });
        handle.on('close', function (closed) { var event = closed && closed.error ? { error: nativeError(closed.error) } : {}; closeListeners.slice().forEach(function (listener) { listener(event); }); });
        return {
          send: function (message) { handle.send(message); },
          onMessage: function (listener) { messageListeners.push(listener); return function () { messageListeners = messageListeners.filter(function (item) { return item !== listener; }); }; },
          onClose: function (listener) { closeListeners.push(listener); return function () { closeListeners = closeListeners.filter(function (item) { return item !== listener; }); }; },
          close: function (error) { handle.close(error); }
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
            if let Some(route) = &child.route {
                out.push_str(&cfg_doc(route, &child_pad));
            }
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
            RouteKind::Watch => {
                let event = rust_to_ts(route.event.as_deref().unwrap_or("unknown"));
                let (params, input) = watch_params(&input_arg, &event);
                format!(
                    "({params}): (() => void) => __watch<{event}>((options) => __dispatch().stream<{event}, void>(\"{}\", {input}, options, pageId), onEvent, options)",
                    route.route
                )
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
            RouteKind::Watch => {
                let event = rust_to_ts(route.event.as_deref().unwrap_or("unknown"));
                let (params, _) = watch_params(&input_arg, &event);
                let open = match &input_ts {
                    Some(input_ts) => format!(
                        "stream<{event}, void, {input_ts}>(\"{}\", input)",
                        route.route
                    ),
                    None => format!("stream<{event}, void>(\"{}\")", route.route),
                };
                format!(
                    "({params}): (() => void) => __watch<{event}>((options) => __toTask({open}, options), onEvent, options)"
                )
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
        RouteKind::Watch => format!("watch([{parts}])"),
        RouteKind::Channel => format!("channel([{parts}])"),
    }
}

/// A watch method's parameters, and the payload expression it sends.
fn watch_params(input_arg: &str, event: &str) -> (String, &'static str) {
    let rest = format!("onEvent: (event: {event}) => void, options?: NativeCallOptions");
    if input_arg.is_empty() {
        (rest, "undefined")
    } else {
        (format!("{input_arg}, {rest}"), "input")
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
        scan_file_into(source, "lib.rs", &mut manifest).unwrap();
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
            pub async fn open_device(input: OpenDeviceInput) -> lingxia::Result<()> { todo!() }
        "#,
        );
        let view = render(&manifest, OutputKind::ViewTypeScript).unwrap();
        assert!(view.contains("device_id: string"), "{view}");
        assert!(view.contains("retry_count?: number | null"), "{view}");
        assert!(view.contains("invoke<void, OpenDeviceInput>"));
        let warnings = wire_warnings(&manifest).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("`OpenDeviceInput` sends `device_id`, `retry_count`"),
            "{warnings:?}"
        );
        assert!(warnings[0].contains("add #[serde(rename_all = \"camelCase\")]"));

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
                progress: Progress<ExportEvent>,
            ) -> lingxia::Result<AdjEvent> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(
            logic.contains(r#"export type ThemeMode = "light-mode" | "dark-mode" | "auto";"#),
            "{logic}"
        );
        assert!(logic.contains(r#"{ kind: "started" }"#), "{logic}");
        assert!(
            logic.contains(r#"{ kind: "progress"; percent: number; note: string }"#),
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
            logic.contains("tags?: Record<string, FlexibleId> | null;"),
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
            pub async fn stat() -> lingxia::Result<MissingDto> { todo!() }
        "#,
        );
        let err = render(&manifest, OutputKind::LogicTypeScript)
            .unwrap_err()
            .to_string();
        assert!(err.contains("MissingDto"), "{err}");
        assert!(err.contains("lib.rs:"), "{err}");
        assert!(err.contains("files.stat"), "{err}");

        let manifest = scan_source(
            r#"
            #[derive(serde::Serialize)]
            pub struct Stat { size: u32, mode: nix::sys::stat::Mode }

            #[lingxia::native("files.stat")]
            pub async fn stat() -> lingxia::Result<Stat> { todo!() }
        "#,
        );
        let err = render(&manifest, OutputKind::LogicTypeScript)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("lib.rs:3: field `mode` of `Stat` has type `Mode`, which codegen cannot map to TypeScript; wrap it in a newtype"),
            "{err}"
        );
    }

    #[test]
    fn sync_route_emits_sync_method_in_logic_and_async_in_view() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("math.add", sync)]
            pub fn add(input: (i32, i32)) -> lingxia::Result<i32> { todo!() }
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
            pub fn scoped(input: String, context: lingxia::host::HostInvocationContext, state: PageState<NotesState>) -> lingxia::Result<()> { todo!() }

            #[lingxia::native("notes.ping")]
            pub fn ping(context: HostInvocationContext, state: PageState<NotesState>) -> lingxia::Result<u32> { todo!() }
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
    fn field_names_follow_serde_and_only_unasked_snake_case_warns() {
        let manifest = scan_source(
            r#"
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Page { page_size: u32, r#type: String }

            #[derive(Serialize)]
            #[serde(rename_all = "snake_case")]
            struct Raw { wire_name: String }

            #[derive(Serialize)]
            #[serde(tag = "kind", rename_all_fields = "camelCase")]
            enum Event { Moved { from_index: u32 }, #[serde(rename_all = "kebab-case")] Named { old_name: String } }

            #[derive(Serialize)]
            enum Loose { Moved { from_index: u32 } }

            struct Unused { never_sent: u32 }

            #[lingxia::native("list.page")]
            pub fn page(input: (Page, Raw)) -> lingxia::Result<(Event, Loose)> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        for expected in [
            "pageSize: number",
            "type: string",
            "wire_name: string",
            "fromIndex: number",
            "\"old-name\": string",
            "from_index: number",
        ] {
            assert!(logic.contains(expected), "{expected}\n{logic}");
        }
        let warnings = wire_warnings(&manifest).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("`Loose` sends `from_index`"),
            "{warnings:?}"
        );
        assert!(warnings[0].contains("rename_all_fields"), "{warnings:?}");
    }

    #[test]
    fn option_fields_are_optional_on_input_and_null_on_output() {
        let manifest = scan_source(
            r#"
            struct Query { limit: Option<u32>, #[serde(default)] page: u32 }
            struct Row { note: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] tag: Option<String>, #[serde(default)] count: u32 }
            struct Both { note: Option<String> }

            #[lingxia::native("rows.list")]
            pub fn list(query: Query) -> lingxia::Result<Vec<Row>> { todo!() }

            #[lingxia::native("rows.echo")]
            pub fn echo(both: Both) -> lingxia::Result<Both> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        for expected in [
            "limit?: number | null;",
            "page?: number;",
            "note: string | null;",
            "tag?: string | null;",
            "count: number;",
            "interface Both {\n  note?: string | null;",
        ] {
            assert!(logic.contains(expected), "{expected}\n{logic}");
        }
    }

    #[test]
    fn clients_follow_the_routes_and_the_requested_view_client() {
        let root = std::env::temp_dir().join(format!(
            "lingxia-native-codegen-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let (src, out) = (root.join("src"), root.join(".lingxia"));
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("lib.rs"),
            r#"
            #[lingxia::framework_native("shell.ping", audience = "control-surface-only")]
            pub fn ping() -> lingxia::Result<()> { todo!() }
        "#,
        )
        .unwrap();
        let exists = |name: &str| out.join(name).exists();

        generate_clients(
            std::slice::from_ref(&src),
            &out,
            Some(ViewClient::TypeScript),
            CallerClass::ControlSurface,
        )
        .unwrap();
        assert!(exists(LOGIC_CLIENT) && exists("native.view.ts"));
        let logic = fs::read_to_string(out.join(LOGIC_CLIENT)).unwrap();
        assert!(logic.contains("\"shell.ping\""), "{logic}");

        let surface = CallerClass::ControlSurface;
        generate_clients(std::slice::from_ref(&src), &out, None, surface).unwrap();
        assert!(exists(LOGIC_CLIENT) && !exists("native.view.ts"));

        // A guest may not call a control-surface route, so it gets no client at all.
        let guest = CallerClass::StandardApp;
        generate_clients(std::slice::from_ref(&src), &out, None, guest).unwrap();
        assert!(!exists(LOGIC_CLIENT));

        fs::write(src.join("lib.rs"), "pub fn plain() {}").unwrap();
        generate_clients(
            std::slice::from_ref(&src),
            &out,
            Some(ViewClient::BrowserGlobal),
            CallerClass::ControlSurface,
        )
        .unwrap();
        assert!(!exists(LOGIC_CLIENT) && !exists("native.js"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dtos_reexported_from_workspace_crates_reach_the_client() {
        let root = std::env::temp_dir().join(format!(
            "lingxia-native-codegen-ws-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"shell\", \"service\", \"transfer\"]\n[workspace.dependencies]\nmy-service = { path = \"service\" }\nserde = \"1\"\n",
        );
        write(
            "shell/Cargo.toml",
            "[package]\nname = \"shell\"\n[dependencies]\nmy-service = { workspace = true }\nserde = { workspace = true }\n",
        );
        write(
            "shell/src/lib.rs",
            r#"
            use my_service::downloads::Snapshot;
            use serde::Opaque;
            #[derive(Serialize)] pub struct IdInput { pub id: String }
            #[lingxia::native("downloads.list")]
            fn list(input: IdInput) -> lingxia::Result<Snapshot> { todo!() }
            #[lingxia::native("downloads.raw")]
            fn raw() -> lingxia::Result<Opaque> { todo!() }
            "#,
        );
        write(
            "service/Cargo.toml",
            "[package]\nname = \"my-service\"\n[dependencies]\ntransfer = { path = \"../transfer\" }\n",
        );
        write("service/src/lib.rs", "pub mod downloads;\n");
        write("service/src/downloads.rs", "pub use transfer::*;\n");
        write("transfer/Cargo.toml", "[package]\nname = \"transfer\"\n");
        write(
            "transfer/src/lib.rs",
            "mod download;\npub use download::{Record, Snapshot};\n",
        );
        write(
            "transfer/src/download.rs",
            r#"
            #[derive(Serialize)] #[serde(rename_all = "camelCase")]
            pub struct Snapshot { pub has_active: bool, pub downloads: Vec<Record>, pub last: IdInput }
            #[derive(Serialize)] pub struct Record { pub id: String }
            #[derive(Serialize)] pub struct IdInput { pub id: String }
            "#,
        );
        let (src, out) = (root.join("shell/src"), root.join("shell/.lingxia"));

        let err = generate_clients(
            std::slice::from_ref(&src),
            &out,
            None,
            CallerClass::StandardApp,
        )
        .unwrap_err();
        assert!(
            format!("{err:#}").contains("`serde::Opaque`, a registry crate"),
            "{err:#}"
        );

        write(
            "shell/src/lib.rs",
            r#"
            use my_service::downloads::Snapshot;
            use serde::Listener;
            #[derive(Serialize)] pub struct IdInput { pub id: String }
            struct Server { listener: Listener }
            #[lingxia::native("downloads.list")]
            fn list(input: IdInput) -> lingxia::Result<Snapshot> { todo!() }
            "#,
        );
        // `Server` holds a registry type, but no route reaches it.
        generate_clients(
            std::slice::from_ref(&src),
            &out,
            None,
            CallerClass::StandardApp,
        )
        .unwrap();
        let logic = fs::read_to_string(out.join(LOGIC_CLIENT)).unwrap();
        assert!(logic.contains("hasActive: boolean"), "{logic}");
        assert!(logic.contains("export interface Record"), "{logic}");

        write(
            "transfer/src/download.rs",
            r#"
            #[derive(Serialize)] #[serde(rename_all = "camelCase")]
            pub struct Snapshot { pub has_active: bool, pub downloads: Vec<Record>, pub last: IdInput }
            #[derive(Serialize)] pub struct Record { pub id: String }
            #[derive(Serialize)] pub struct IdInput { pub key: u32 }
            "#,
        );
        let err = generate_clients(
            std::slice::from_ref(&src),
            &out,
            None,
            CallerClass::StandardApp,
        )
        .unwrap_err();
        let err = format!("{err:#}");
        assert!(
            err.contains("`IdInput`")
                && err.contains("lib.rs")
                && err.contains("transfer/src/download.rs"),
            "{err}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn scan_files(files: &[(&str, &str)]) -> NativeManifest {
        let mut manifest = NativeManifest::default();
        for (label, source) in files {
            scan_file_into(source, label, &mut manifest).unwrap();
        }
        manifest
    }

    #[test]
    fn the_registry_names_each_route_by_its_module_and_keeps_its_cfgs() {
        let manifest = scan_files(&[
            (
                "lib.rs",
                r#"
                #[cfg(feature = "net")]
                mod net;
                mod tools { pub(crate) mod inner {
                    #[lingxia::native("tools.ping")]
                    #[cfg(unix)]
                    fn ping() -> lingxia::Result<()> { todo!() }
                } }
            "#,
            ),
            ("net/mod.rs", "pub(crate) mod proxy;"),
            (
                "net/proxy.rs",
                r#"
                #[lingxia::framework_native("net.get", audience = "control-surface-only")]
                fn get() -> lingxia::Result<()> { todo!() }
            "#,
            ),
        ]);
        let registry = render(&manifest, OutputKind::RustModule).unwrap();
        let net = registry
            .find(
                "crate::net::proxy::__lingxia_route_get(\"net\", \"get\", ::lingxia::host::RouteAudience::ControlSurfaceOnly)",
            )
            .expect(&registry);
        let feature = registry
            .find("# [cfg (feature = \"net\")]")
            .expect(&registry);
        assert!(feature < net, "{registry}");
        let ping = registry
            .find(
                "crate::tools::inner::__lingxia_route_ping(\"tools\", \"ping\", ::lingxia::host::RouteAudience::AppSessionOnly)",
            )
            .expect(&registry);
        let unix = registry.find("# [cfg (unix)]").expect(&registry);
        assert!(unix < ping && ping < feature, "{registry}");
    }

    #[test]
    fn a_route_the_crate_root_cannot_reach_is_an_error_not_a_missing_route() {
        let private = scan_files(&[
            ("lib.rs", "mod net;"),
            ("net.rs", "mod proxy;"),
            (
                "net/proxy.rs",
                "#[lingxia::native(\"net.get\")]\nfn get() -> lingxia::Result<()> { todo!() }",
            ),
        ]);
        let err = render(&private, OutputKind::RustModule)
            .unwrap_err()
            .to_string();
        assert!(err.contains("net/proxy.rs:2"), "{err}");
        assert!(err.contains("pub(crate) mod proxy"), "{err}");

        let undeclared = scan_files(&[(
            "stray.rs",
            "#[lingxia::native(\"stray.get\")]\nfn get() -> lingxia::Result<()> { todo!() }",
        )]);
        let err = render(&undeclared, OutputKind::RustModule)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no `mod` declares"), "{err}");
    }

    #[test]
    fn parameters_are_classified_by_type_in_any_order() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("apps.describe")]
            pub fn describe(cancel: HostCancel, info: LxAppInfo, app: Arc<LxApp>) -> lingxia::Result<()> { todo!() }

            #[lingxia::native("apps.watch", stream)]
            pub async fn watch(progress: Progress<AppEvent>, query: AppQuery) -> lingxia::Result<()> { todo!() }
        "#,
        );
        let route = |name: &str| manifest.routes.iter().find(|r| r.route == name).unwrap();
        assert_eq!(route("apps.describe").input.as_deref(), Some("LxAppInfo"));
        assert_eq!(route("apps.watch").input.as_deref(), Some("AppQuery"));
        assert_eq!(route("apps.watch").event.as_deref(), Some("AppEvent"));
    }

    #[test]
    fn a_stream_result_is_its_return_value_and_a_watch_reconnects() {
        let manifest = scan_source(
            r#"
            #[derive(serde::Serialize)]
            pub struct Step { done: u32 }
            #[derive(serde::Serialize)]
            pub struct Summary { rows: u32 }
            #[derive(serde::Deserialize)]
            pub struct Filter { tag: String }

            #[lingxia::native("notes.import", stream)]
            pub async fn import(progress: Progress<Step>) -> Result<Summary, NotesError> { todo!() }

            #[lingxia::native("notes.watch", watch)]
            pub async fn follow(app: Arc<LxApp>) -> lingxia::Result<impl Stream<Item = Step> + Send + 'static> { todo!() }

            #[lingxia::native("notes.watchTagged", watch)]
            pub fn follow_tagged(filter: Filter) -> lingxia::Result<impl futures::Stream<Item = Summary>> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(
            logic.contains("import: (options?: NativeCallOptions): NativeTask<Step, Summary> =>"),
            "{logic}"
        );
        assert!(
            logic.contains("watch: (onEvent: (event: Step) => void, options?: NativeCallOptions): (() => void) => __watch<Step>((options) => __dispatch().stream<Step, void>(\"notes.watch\", undefined, options, pageId), onEvent, options)"),
            "{logic}"
        );
        assert!(
            logic.contains("watchTagged: (input: Filter, onEvent: (event: Summary) => void, options?: NativeCallOptions): (() => void) => __watch<Summary>((options) => __dispatch().stream<Summary, void>(\"notes.watchTagged\", input, options, pageId)"),
            "{logic}"
        );
        assert!(logic.contains("function __watch<E>("), "{logic}");

        let view = render(&manifest, OutputKind::ViewTypeScript).unwrap();
        assert!(
            view.contains("__watch<Summary>((options) => __toTask(stream<Summary, void, Filter>(\"notes.watchTagged\", input), options), onEvent, options)"),
            "{view}"
        );
        let js = render(&manifest, OutputKind::BrowserGlobalJs).unwrap();
        assert!(
            js.contains("watchTagged: watch([\"notes\", \"watchTagged\"])"),
            "{js}"
        );

        let without = scan_source(
            r#"
            #[lingxia::native("notes.get")]
            pub fn get() -> lingxia::Result<()> { todo!() }
        "#,
        );
        let logic = render(&without, OutputKind::LogicTypeScript).unwrap();
        assert!(!logic.contains("__watch"), "{logic}");
    }

    #[test]
    fn a_watch_that_does_not_return_a_stream_fails_with_its_location() {
        let mut manifest = NativeManifest::default();
        let err = scan_file_into(
            r#"
            #[lingxia::native("notes.watch", watch)]
            pub async fn follow() -> lingxia::Result<Vec<String>> { todo!() }
        "#,
            "src/notes.rs",
            &mut manifest,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("src/notes.rs:"), "{err}");
        assert!(err.contains("impl Stream<Item = Event>"), "{err}");
    }

    #[test]
    fn a_route_the_macro_would_reject_fails_with_its_location() {
        let mut manifest = NativeManifest::default();
        let err = scan_file_into(
            r#"
            #[lingxia::native("apps.pair")]
            pub fn pair(a: Left, b: Right) -> lingxia::Result<()> { todo!() }
        "#,
            "src/apps.rs",
            &mut manifest,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("src/apps.rs:"), "{err}");
        assert!(err.contains("`apps.pair`"), "{err}");
        assert!(err.contains("one payload argument"), "{err}");
    }

    #[test]
    fn a_native_error_enum_becomes_a_union_with_its_data_and_a_guard() {
        let manifest = scan_source(
            r#"
            #[derive(Debug, thiserror::Error, Serialize, lingxia::NativeError)]
            #[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
            enum NotesError {
                #[error("note {id} not found")]
                NotFound { id: i64 },
                #[error("locked")]
                Locked,
                #[error(transparent)]
                #[serde(skip)]
                #[native_error(internal)]
                Db(#[from] DbError),
            }

            #[lingxia::native("notes.delete")]
            pub async fn delete(id: i64) -> Result<(), NotesError> { todo!() }
        "#,
        );
        for kind in [OutputKind::LogicTypeScript, OutputKind::ViewTypeScript] {
            let ts = render(&manifest, kind).unwrap();
            assert!(
                ts.contains(
                    "export type NotesError = { code: \"NOT_FOUND\"; message: string; reason?: string; data: { id: number } } \
                     | { code: \"LOCKED\"; message: string; reason?: string } | FrameworkError;"
                ),
                "{ts}"
            );
            assert!(
                ts.contains("export type AppNativeError = Exclude<NotesError, FrameworkError>;"),
                "{ts}"
            );
            assert!(
                ts.contains("export function isNativeError<C extends NativeErrorCode"),
                "{ts}"
            );
            assert!(ts.contains("code: `E_${string}`;"), "{ts}");
        }
    }

    #[test]
    fn a_native_error_enum_the_derive_would_reject_fails_with_its_location() {
        let cases = [
            ("enum E { A }", "needs `#[serde(tag = \"code\")]`"),
            (
                "#[serde(tag = \"code\")] enum E { #[serde(rename = \"E_GONE\")] Gone }",
                "belong to the framework",
            ),
            (
                "#[serde(tag = \"code\")] enum E { #[native_error(internal)] Db(DbError) }",
                "add `#[serde(skip)]`",
            ),
            (
                "#[serde(tag = \"code\")] enum E { #[serde(skip)] Db(DbError) }",
                "mark it `#[native_error(internal)]`",
            ),
            (
                "#[serde(tag = \"code\")] enum E { Pair(u8, u8) }",
                "use named fields",
            ),
        ];
        for (body, expected) in cases {
            let source = format!("#[derive(serde::Serialize, lingxia::NativeError)] {body}");
            let err = scan_file_into(&source, "src/errors.rs", &mut NativeManifest::default())
                .unwrap_err()
                .to_string();
            assert!(err.contains("src/errors.rs:"), "{err}");
            assert!(err.contains(expected), "{body}: {err}");
        }
    }

    #[test]
    fn logic_client_types_each_mockable_route_for_native_mocks() {
        let manifest = scan_source(
            r#"
            #[derive(Debug, thiserror::Error, Serialize, lingxia::NativeError)]
            #[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
            enum NotesError {
                #[error("locked")]
                Locked,
            }

            #[lingxia::native("notes.delete")]
            pub async fn delete(id: i64) -> Result<(), crate::NotesError> { todo!() }

            #[lingxia::native("notes.list")]
            pub fn list() -> lingxia::Result<Vec<String>> { todo!() }

            #[lingxia::native("notes.import", stream)]
            pub async fn import(path: String, progress: Progress<u32>) -> lingxia::Result<u64> { todo!() }

            #[lingxia::native("notes.shout", channel)]
            pub async fn shout(channel: ChannelContext<String>) -> lingxia::Result<()> { todo!() }
        "#,
        );
        let ts = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        for line in [
            "declare module \"@lingxia/types/mocks\" {",
            "\"notes.delete\": { input: number; result: void; event: never; error: NotesError };",
            "\"notes.list\": { input: undefined; result: string[]; event: never; error: FrameworkError };",
            "\"notes.import\": { input: string; result: number; event: number; error: FrameworkError };",
        ] {
            assert!(ts.contains(line), "{line}\n{ts}");
        }
        assert!(!ts.contains("\"notes.shout\": { input"), "{ts}");
    }

    #[test]
    fn without_native_errors_the_guard_narrows_framework_codes_only() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("device.close")]
            pub fn close_device() -> lingxia::Result<()> { todo!() }
        "#,
        );
        let ts = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(ts.contains("export type AppNativeError = never;"), "{ts}");
    }

    #[test]
    fn generated_browser_js_exposes_unified_async_iterable_task() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("downloads.watch", stream)]
            pub async fn watch(progress: Progress<String>) -> lingxia::Result<()> { todo!() }
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
            pub fn open_device(input: String) -> lingxia::Result<()> { todo!() }

            #[lingxia::native("device.close")]
            pub fn close_device() -> lingxia::Result<()> { todo!() }
        "#,
        );
        let generated = render(&manifest, OutputKind::RustModule).unwrap();
        assert!(generated.contains("mod __lingxia_native"));
        assert!(generated.contains(
            "::lingxia::host::register_host_entry(crate::__lingxia_route_open_device(\"device\", \"open\", ::lingxia::host::RouteAudience::AppSessionOnly))"
        ));
        assert!(generated.contains(
            "::lingxia::host::register_host_entry(crate::__lingxia_route_close_device(\"device\", \"close\", ::lingxia::host::RouteAudience::AppSessionOnly))"
        ));
    }

    fn scan_error(source: &str) -> String {
        let mut manifest = NativeManifest::default();
        scan_file_into(source, "lib.rs", &mut manifest)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn route_defaults_name_and_scope_the_rest_of_the_file() {
        let manifest = scan_source(
            r#"
            #[lingxia::native("early.route")]
            fn early() -> lingxia::Result<()> { todo!() }

            lingxia::route_defaults!(prefix = "bookmarks", audience = "control-surface-only");

            #[lingxia::framework_native]
            fn set_pinned() -> lingxia::Result<()> { todo!() }
            #[lingxia::native("export_html")]
            fn export() -> lingxia::Result<()> { todo!() }
            #[lingxia::native("other.absolute", audience = "app-session-only")]
            fn absolute() -> lingxia::Result<()> { todo!() }

            mod child {
                #[lingxia::native("child.ping")]
                fn ping() -> lingxia::Result<()> { todo!() }
            }
        "#,
        );
        let routes: Vec<_> = manifest
            .routes
            .iter()
            .map(|r| (r.route.as_str(), r.audience))
            .collect();
        assert_eq!(
            routes,
            [
                ("bookmarks.export_html", RouteAudience::ControlSurfaceOnly),
                ("bookmarks.setPinned", RouteAudience::ControlSurfaceOnly),
                ("child.ping", RouteAudience::AppSessionOnly),
                ("early.route", RouteAudience::AppSessionOnly),
                ("other.absolute", RouteAudience::AppSessionOnly),
            ]
        );
    }

    #[test]
    fn route_defaults_misuse_and_unnamed_routes_are_errors() {
        let second = scan_error(
            "lingxia::route_defaults!(prefix = \"a\");\nroute_defaults!(prefix = \"b\");",
        );
        assert!(
            second.contains("lib.rs:2: a second route_defaults!"),
            "{second}"
        );
        assert!(second.contains("first is at line 1"), "{second}");

        let nested = scan_error("mod inner { lingxia::route_defaults!(prefix = \"a\"); }");
        assert!(nested.contains("inline `mod`"), "{nested}");

        let unnamed =
            scan_error("#[lingxia::native]\nfn load() -> lingxia::Result<()> { todo!() }");
        assert!(
            unnamed.contains("lib.rs:2: `load` has no route name"),
            "{unnamed}"
        );

        let bare = scan_error(
            "#[lingxia::native(\"load\")]\nfn load() -> lingxia::Result<()> { todo!() }",
        );
        assert!(bare.contains("route `load` has no namespace"), "{bare}");

        let framework = scan_error(
            "lingxia::route_defaults!(prefix = \"fw\");\n#[lingxia::framework_native]\nfn load() -> lingxia::Result<()> { todo!() }",
        );
        assert!(
            framework.contains("framework route `fw.load` needs `audience"),
            "{framework}"
        );
    }

    #[test]
    fn test_only_code_never_reaches_a_client() {
        let manifest = scan_source(
            r#"
            #[derive(serde::Serialize)]
            pub struct Note { title: String }

            #[lingxia::native("notes.get")]
            pub fn get() -> lingxia::Result<Note> { todo!() }

            #[cfg(test)]
            mod tests {
                struct Note { id: u32, body: Vec<u8> }

                #[lingxia::native("notes.fixture")]
                fn fixture() -> lingxia::Result<Note> { todo!() }
            }

            #[cfg(test)]
            #[lingxia::native("notes.probe")]
            fn probe() -> lingxia::Result<()> { todo!() }
        "#,
        );
        let routes: Vec<_> = manifest.routes.iter().map(|r| r.route.as_str()).collect();
        assert_eq!(routes, ["notes.get"]);
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(logic.contains("title: string;"), "{logic}");
    }

    #[test]
    fn outside_string_types_and_wide_integers() {
        let manifest = scan_source(
            r#"
            use chrono::{DateTime, Utc};
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            pub struct Upload {
                id: uuid::Uuid,
                path: std::path::PathBuf,
                at: DateTime<Utc>,
                source: Option<url::Url>,
                bytes: u64,
                chunks: Vec<i64>,
                count: u32,
            }

            #[lingxia::native("files.upload")]
            pub fn upload() -> lingxia::Result<Upload> { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        for field in [
            "id: string;",
            "path: string;",
            "at: string;",
            "source: string | null;",
        ] {
            assert!(logic.contains(field), "{field}: {logic}");
        }
        let warnings = wire_warnings(&manifest).unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("`bytes`, `chunks`"), "{warnings:?}");
        assert!(warnings[0].contains("2^53"), "{warnings:?}");
    }

    #[test]
    fn a_cfg_gated_route_says_where_it_exists() {
        let manifest = scan_source(
            r#"
            #[cfg(target_os = "android")]
            #[lingxia::native("device.vibrate")]
            pub fn vibrate() -> lingxia::Result<()> { todo!() }

            #[lingxia::native("device.name")]
            pub fn name() -> String { todo!() }
        "#,
        );
        let logic = render(&manifest, OutputKind::LogicTypeScript).unwrap();
        assert!(
            logic.contains("/** Only where `cfg(target_os = \"android\")`; elsewhere the call fails with `E_NOT_FOUND`. */\n      vibrate:"),
            "{logic}"
        );
        assert!(logic.contains("name: (): Promise<string>"), "{logic}");
        assert!(!logic.contains("Only where `cfg(target_os = \"android\")`; elsewhere the call fails with `E_NOT_FOUND`. */\n      name"), "{logic}");
    }

    #[test]
    fn a_client_carries_only_the_routes_its_caller_class_may_call() {
        let root =
            std::env::temp_dir().join(format!("lingxia-native-class-{}", std::process::id()));
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        let route = |name: &str, audience: &str| {
            format!(
                "#[lingxia::native(\"{name}\", audience = \"{audience}\")]\npub fn {}() -> u32 {{ 0 }}\n",
                name.replace('.', "_")
            )
        };
        let lib = [
            route("all.any", "app-session-only"),
            route("home.only", "control-app-only"),
            route("surface.only", "control-surface-only"),
            route("control.both", "control-app-or-surface"),
        ]
        .concat();
        fs::write(src.join("lib.rs"), lib).unwrap();
        let out = root.join("out");
        let routes = |caller| {
            generate_clients(std::slice::from_ref(&src), &out, None, caller).unwrap();
            let logic = fs::read_to_string(out.join(LOGIC_CLIENT)).unwrap();
            ["all.any", "home.only", "surface.only", "control.both"]
                .into_iter()
                .filter(|name| logic.contains(&format!("\"{name}\"")))
                .collect::<Vec<_>>()
        };
        let home = routes(CallerClass::ControlApp);
        let surface = routes(CallerClass::ControlSurface);
        let guest = routes(CallerClass::StandardApp);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(home, ["all.any", "home.only", "control.both"]);
        assert_eq!(surface, ["all.any", "surface.only", "control.both"]);
        assert_eq!(guest, ["all.any"]);
    }

    #[test]
    fn library_crates_join_one_client_and_collisions_name_both_crates() {
        let root = std::env::temp_dir().join(format!("lingxia-native-libs-{}", std::process::id()));
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("host/Cargo.toml", "[package]\nname = \"host\"\n");
        write(
            "host/src/lib.rs",
            "lingxia::host!(App, routes = [notes_core]);\n#[lingxia::native(\"app.ping\")]\npub fn ping() -> String { todo!() }\n",
        );
        write(
            "notes-core/Cargo.toml",
            "[package]\nname = \"notes-core\"\n",
        );
        write(
            "notes-core/src/lib.rs",
            "lingxia::routes!();\n#[derive(serde::Serialize)] pub struct Note { pub id: u32 }\n#[lingxia::native(\"notes.get\")]\npub fn get() -> Note { todo!() }\n",
        );
        let (host, notes) = (root.join("host/src"), root.join("notes-core/src"));
        let out = root.join("out");
        generate_clients(
            &[host.clone(), notes.clone()],
            &out,
            None,
            CallerClass::StandardApp,
        )
        .unwrap();
        let logic = fs::read_to_string(out.join(LOGIC_CLIENT)).unwrap();
        assert!(
            logic.contains("\"app.ping\"") && logic.contains("\"notes.get\""),
            "{logic}"
        );
        assert!(declares_routes(&notes.join("lib.rs")).unwrap());
        assert!(!declares_routes(&host.join("lib.rs")).unwrap());
        assert_eq!(
            host_route_crates(&host.join("lib.rs")).unwrap(),
            Some(vec!["notes_core".to_string()])
        );
        assert_eq!(host_route_crates(&notes.join("lib.rs")).unwrap(), None);

        write(
            "host/src/lib.rs",
            "#[lingxia::native(\"notes.get\")]\npub fn get() -> String { todo!() }\n",
        );
        let err = generate_clients(
            &[host.clone(), notes.clone()],
            &out,
            None,
            CallerClass::StandardApp,
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("`notes.get` is declared twice, at lib.rs:2 and notes-core/src/lib.rs:4"),
            "{err}"
        );

        write(
            "host/src/lib.rs",
            "#[derive(serde::Serialize)] pub struct Note { pub title: String }\n#[lingxia::native(\"app.note\")]\npub fn note() -> Note { todo!() }\n",
        );
        let err = format!(
            "{:#}",
            generate_clients(&[host, notes], &out, None, CallerClass::StandardApp).unwrap_err()
        );
        let _ = fs::remove_dir_all(&root);
        assert!(
            err.contains("`Note`") && err.contains("notes-core/src/lib.rs"),
            "{err}"
        );
    }

    #[test]
    fn a_route_declared_twice_names_both_places() {
        let root = std::env::temp_dir().join(format!("lingxia-native-dup-{}", std::process::id()));
        let src = root.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("lib.rs"),
            "mod a;\n#[lingxia::native(\"notes.get\")]\npub fn get() -> String { todo!() }\n",
        )
        .unwrap();
        fs::write(
            src.join("a.rs"),
            "\n#[lingxia::native(\"notes.get\")]\npub fn other() -> String { todo!() }\n",
        )
        .unwrap();
        let err = scan(&src).unwrap_err().to_string();
        let _ = fs::remove_dir_all(&root);
        assert!(
            err.contains("`notes.get` is declared twice, at a.rs:3 and lib.rs:3"),
            "{err}"
        );

        // Two fn names that camelCase to one route under the same prefix.
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("lib.rs"),
            "lingxia::route_defaults!(prefix = \"bookmarks\");\n#[lingxia::native]\npub fn set_pinned() {}\n#[lingxia::native(\"setPinned\")]\npub fn pin() {}\n",
        )
        .unwrap();
        let err = scan(&src).unwrap_err().to_string();
        let _ = fs::remove_dir_all(&root);
        assert!(
            err.contains("`bookmarks.setPinned` is declared twice, at lib.rs:3 and lib.rs:5"),
            "{err}"
        );
    }
}
