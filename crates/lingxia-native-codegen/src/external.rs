//! DTOs a route names from another workspace crate: found by following the native crate's
//! `use` into the dependency's sources, through its `pub use` re-exports, across path
//! dependencies. A type from a registry crate has no sources to read, so it stays an error.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use syn::ext::IdentExt;

use crate::{
    NativeManifest, SourceLoc, TypeAliasDef, collect_refs_from_type, extract_enum_def,
    extract_struct_def, insert_def, type_string,
};

/// A path from a `use`: its segments, and `true` for a `*` import of them.
#[derive(Debug, Clone)]
pub(crate) struct UsePath {
    pub(crate) segments: Vec<String>,
    pub(crate) glob: bool,
}

/// Every import a `use` item makes, keyed by the name it brings in (`*` for a glob).
pub(crate) fn flatten_use(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    out: &mut Vec<(String, UsePath)>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.unraw().to_string());
            flatten_use(&path.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.unraw().to_string();
            if ident != "self" {
                let mut segments = prefix.clone();
                segments.push(ident.clone());
                out.push((
                    ident,
                    UsePath {
                        segments,
                        glob: false,
                    },
                ));
            }
        }
        syn::UseTree::Rename(rename) => {
            let mut segments = prefix.clone();
            segments.push(rename.ident.unraw().to_string());
            out.push((
                rename.rename.unraw().to_string(),
                UsePath {
                    segments,
                    glob: false,
                },
            ));
        }
        syn::UseTree::Glob(_) => out.push((
            "*".to_string(),
            UsePath {
                segments: prefix.clone(),
                glob: true,
            },
        )),
        syn::UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(item, prefix, out);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Dep {
    Path(PathBuf),
    Registry,
}

/// A module of some crate: its items and where its child modules' files live.
struct Module {
    crate_dir: PathBuf,
    path: Vec<String>,
    items: Vec<syn::Item>,
    file: PathBuf,
    child_dir: PathBuf,
}

struct Resolver {
    workspace_root: Option<PathBuf>,
    deps: BTreeMap<PathBuf, BTreeMap<String, Dep>>,
}

/// Where a name is looked up: the native crate's own imports, or a module of a dependency.
#[derive(Clone)]
enum Scope {
    Native,
    Module {
        crate_dir: PathBuf,
        path: Vec<String>,
    },
}

const MAX_HOPS: usize = 24;

/// Adds to `manifest` every DTO its routes reach that the native crate imports from a
/// workspace path dependency.
pub(crate) fn resolve(manifest: &mut NativeManifest, src_dir: &Path) -> Result<()> {
    let crate_dir = src_dir.parent().unwrap_or(src_dir).to_path_buf();
    let mut resolver = Resolver {
        workspace_root: crate_dir
            .ancestors()
            .skip(1)
            .find(|dir| is_workspace_root(dir))
            .map(Path::to_path_buf),
        deps: BTreeMap::new(),
    };
    // Only what a route reaches: a private struct may hold anything.
    let mut pending: Vec<(String, Scope)> = crate::route_refs(manifest)
        .into_iter()
        .map(|name| (name, Scope::Native))
        .collect();
    let mut seen = BTreeSet::new();
    while let Some((name, scope)) = pending.pop() {
        let key = match &scope {
            Scope::Native => format!("::{name}"),
            Scope::Module { crate_dir, path } => {
                format!("{}::{}::{name}", crate_dir.display(), path.join("::"))
            }
        };
        if !seen.insert(key) {
            continue;
        }
        let found = match &scope {
            Scope::Native => {
                if manifest.defines(&name) {
                    let refs = crate::def_refs(manifest, &name);
                    pending.extend(refs.into_iter().map(|child| (child, Scope::Native)));
                    continue;
                }
                let Some(target) = manifest.uses.get(&name).cloned() else {
                    continue;
                };
                let Some((dep, rest)) = target.segments.split_first() else {
                    continue;
                };
                if matches!(dep.as_str(), "crate" | "self" | "super") {
                    continue;
                }
                match resolver.dep(&crate_dir, dep)? {
                    Some(Dep::Path(dir)) => resolver.find(&dir, rest, 0)?,
                    Some(Dep::Registry) => bail!(
                        "native DTO `{name}` comes from `{}`, a registry crate whose sources the CLI does not read; define the DTO in the native crate",
                        target.segments.join("::")
                    ),
                    None => continue,
                }
            }
            Scope::Module { crate_dir, path } => {
                let module = resolver.module(crate_dir, path)?;
                resolver.lookup(&module, &name, 0)?
            }
        };
        let Some((item, module)) = found else {
            continue;
        };
        let label = resolver.label(&module.file);
        let refs = add_item(manifest, &name, &item, &label)?;
        let scope = Scope::Module {
            crate_dir: module.crate_dir.clone(),
            path: module.path.clone(),
        };
        pending.extend(refs.into_iter().map(|child| (child, scope.clone())));
    }
    Ok(())
}

/// Inserts `item` under `name` and returns the type names its fields refer to.
fn add_item(
    manifest: &mut NativeManifest,
    name: &str,
    item: &syn::Item,
    label: &str,
) -> Result<BTreeSet<String>> {
    let mut refs = BTreeSet::new();
    let loc = SourceLoc {
        file: label.to_string(),
        line: 0,
    };
    match item {
        syn::Item::Struct(item) => {
            if let Some(def) = extract_struct_def(item, loc) {
                def.refs(&mut refs);
                insert_def(manifest, name, crate::Def::Struct(def))?;
            }
        }
        syn::Item::Enum(item) => {
            if let Some(def) = extract_enum_def(item, loc) {
                def.refs(&mut refs);
                insert_def(manifest, name, crate::Def::Enum(def))?;
            }
        }
        syn::Item::Type(item) => {
            let target = type_string(&item.ty).replace(' ', "");
            collect_refs_from_type(&target, &mut refs);
            insert_def(
                manifest,
                name,
                crate::Def::Alias(TypeAliasDef { target, loc }),
            )?;
        }
        _ => {}
    }
    Ok(refs)
}

fn is_workspace_root(dir: &Path) -> bool {
    fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|table| table.contains_key("workspace"))
}

impl Resolver {
    /// What the crate at `crate_dir` calls `name`: a path dependency, a registry one, or
    /// nothing (a local module or a std path).
    fn dep(&mut self, crate_dir: &Path, name: &str) -> Result<Option<Dep>> {
        if !self.deps.contains_key(crate_dir) {
            let deps = self.read_deps(crate_dir)?;
            self.deps.insert(crate_dir.to_path_buf(), deps);
        }
        Ok(self.deps[crate_dir].get(name).cloned())
    }

    fn read_deps(&self, crate_dir: &Path) -> Result<BTreeMap<String, Dep>> {
        let manifest_path = crate_dir.join("Cargo.toml");
        let text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        let table: toml::Table = text
            .parse()
            .with_context(|| format!("parse {}", manifest_path.display()))?;
        let workspace = self.workspace_deps()?;
        let mut tables: Vec<&toml::Table> = table
            .get("dependencies")
            .and_then(|v| v.as_table())
            .into_iter()
            .collect();
        if let Some(targets) = table.get("target").and_then(|v| v.as_table()) {
            for target in targets.values() {
                if let Some(deps) = target.get("dependencies").and_then(|v| v.as_table()) {
                    tables.push(deps);
                }
            }
        }
        let mut out = BTreeMap::new();
        for deps in tables {
            for (key, value) in deps {
                let dep = match value.as_table() {
                    Some(spec) if spec.get("workspace").and_then(|v| v.as_bool()) == Some(true) => {
                        workspace.get(key).cloned().unwrap_or(Dep::Registry)
                    }
                    Some(spec) => match spec.get("path").and_then(|v| v.as_str()) {
                        Some(path) => Dep::Path(crate_dir.join(path)),
                        None => Dep::Registry,
                    },
                    None => Dep::Registry,
                };
                out.insert(key.replace('-', "_"), dep);
            }
        }
        Ok(out)
    }

    fn workspace_deps(&self) -> Result<BTreeMap<String, Dep>> {
        let Some(root) = &self.workspace_root else {
            return Ok(BTreeMap::new());
        };
        let manifest_path = root.join("Cargo.toml");
        let table: toml::Table = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?
            .parse()
            .with_context(|| format!("parse {}", manifest_path.display()))?;
        let deps = table
            .get("workspace")
            .and_then(|v| v.get("dependencies"))
            .and_then(|v| v.as_table());
        Ok(deps
            .into_iter()
            .flatten()
            .map(|(key, value)| {
                let dep = match value.get("path").and_then(|v| v.as_str()) {
                    Some(path) => Dep::Path(root.join(path)),
                    None => Dep::Registry,
                };
                (key.clone(), dep)
            })
            .collect())
    }

    fn label(&self, file: &Path) -> String {
        let base = self.workspace_root.as_deref().unwrap_or(Path::new(""));
        file.strip_prefix(base)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// The module at `path` from the root of the crate at `crate_dir`.
    fn module(&self, crate_dir: &Path, path: &[String]) -> Result<Module> {
        let file = crate_dir.join("src/lib.rs");
        let mut module = Module {
            crate_dir: crate_dir.to_path_buf(),
            path: Vec::new(),
            items: parse(&file)?,
            child_dir: crate_dir.join("src"),
            file,
        };
        for segment in path {
            module = child(module, segment)?.ok_or_else(|| {
                anyhow!("no module `{}` in {}", path.join("::"), crate_dir.display())
            })?;
        }
        Ok(module)
    }

    /// `segments` (module path, then the name) from the root of the crate at `crate_dir`.
    fn find(
        &mut self,
        crate_dir: &Path,
        segments: &[String],
        hops: usize,
    ) -> Result<Option<(syn::Item, Module)>> {
        let Some((name, modules)) = segments.split_last() else {
            return Ok(None);
        };
        let mut module = match self.module(crate_dir, &[]) {
            Ok(module) => module,
            Err(_) => return Ok(None),
        };
        for segment in modules {
            match child(module, segment)? {
                Some(next) => module = next,
                None => return Ok(None),
            }
        }
        self.lookup(&module, name, hops)
    }

    /// `name` as `module` sees it: an item of its own, or one it imports.
    fn lookup(
        &mut self,
        module: &Module,
        name: &str,
        hops: usize,
    ) -> Result<Option<(syn::Item, Module)>> {
        if hops > MAX_HOPS {
            bail!("re-exports of `{name}` go round in circles");
        }
        for item in &module.items {
            let ident = match item {
                syn::Item::Struct(item) => &item.ident,
                syn::Item::Enum(item) => &item.ident,
                syn::Item::Type(item) => &item.ident,
                _ => continue,
            };
            if ident.unraw() == name {
                let owner = Module {
                    crate_dir: module.crate_dir.clone(),
                    path: module.path.clone(),
                    items: module.items.clone(),
                    file: module.file.clone(),
                    child_dir: module.child_dir.clone(),
                };
                return Ok(Some((item.clone(), owner)));
            }
        }
        let mut imports = Vec::new();
        for item in &module.items {
            if let syn::Item::Use(item) = item {
                flatten_use(&item.tree, &mut Vec::new(), &mut imports);
            }
        }
        let named = imports
            .iter()
            .filter(|(alias, path)| !path.glob && alias == name);
        let globs = imports.iter().filter(|(_, path)| path.glob);
        for (_, target) in named.chain(globs) {
            let mut segments = target.segments.clone();
            if target.glob {
                segments.push(name.to_string());
            }
            if let Some(found) = self.follow(module, &segments, hops + 1)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// A `use` path written in `module`, resolved to the item it names.
    fn follow(
        &mut self,
        module: &Module,
        segments: &[String],
        hops: usize,
    ) -> Result<Option<(syn::Item, Module)>> {
        let Some((first, rest)) = segments.split_first() else {
            return Ok(None);
        };
        match first.as_str() {
            "crate" => self.find(&module.crate_dir.clone(), rest, hops),
            "self" => {
                let mut path = module.path.clone();
                path.extend(rest.iter().cloned());
                self.find(&module.crate_dir.clone(), &path, hops)
            }
            "super" => {
                let mut path = module.path.clone();
                path.pop();
                path.extend(rest.iter().cloned());
                self.find(&module.crate_dir.clone(), &path, hops)
            }
            dep => match self.dep(&module.crate_dir.clone(), dep)? {
                Some(Dep::Path(dir)) => self.find(&dir, rest, hops),
                Some(Dep::Registry) => bail!(
                    "native DTO `{}` is re-exported from `{}`, a registry crate whose sources the CLI does not read; define the DTO in a workspace crate",
                    segments.last().map(String::as_str).unwrap_or_default(),
                    segments.join("::")
                ),
                None => {
                    let mut path = module.path.clone();
                    path.extend(segments.iter().cloned());
                    self.find(&module.crate_dir.clone(), &path, hops)
                }
            },
        }
    }
}

fn parse(file: &Path) -> Result<Vec<syn::Item>> {
    let source = fs::read_to_string(file).with_context(|| format!("read {}", file.display()))?;
    Ok(syn::parse_file(&source)
        .with_context(|| format!("parse {}", file.display()))?
        .items)
}

/// The child module `name` of `module`: inline, `name.rs` or `name/mod.rs`.
fn child(module: Module, name: &str) -> Result<Option<Module>> {
    for item in &module.items {
        let syn::Item::Mod(item) = item else { continue };
        if item.ident.unraw() != name {
            continue;
        }
        let mut path = module.path.clone();
        path.push(name.to_string());
        if let Some((_, items)) = &item.content {
            return Ok(Some(Module {
                crate_dir: module.crate_dir.clone(),
                path,
                items: items.clone(),
                file: module.file.clone(),
                child_dir: module.child_dir.join(name),
            }));
        }
        for (file, child_dir) in [
            (
                module.child_dir.join(format!("{name}.rs")),
                module.child_dir.join(name),
            ),
            (
                module.child_dir.join(name).join("mod.rs"),
                module.child_dir.join(name),
            ),
        ] {
            if file.is_file() {
                return Ok(Some(Module {
                    crate_dir: module.crate_dir.clone(),
                    path,
                    items: parse(&file)?,
                    file,
                    child_dir,
                }));
            }
        }
        return Ok(None);
    }
    Ok(None)
}
