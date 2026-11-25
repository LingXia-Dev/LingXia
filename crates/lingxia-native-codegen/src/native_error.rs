//! Reads a `#[derive(NativeError)]` enum. The derive and the TypeScript codegen both read it
//! here, so the codes the derive sends are the codes the generated union names.

use syn::spanned::Spanned;
use syn::{Attribute, Fields, ItemEnum};

use super::{RenameRule, parse_serde_container_attrs, parse_serde_member_attrs};

/// One variant: its app code, or `None` when it is `#[native_error(internal)]`.
#[derive(Debug, Clone)]
pub struct NativeErrorVariant {
    pub ident: syn::Ident,
    pub code: Option<String>,
}

/// True when `attrs` hold a `#[derive(..)]` naming `NativeError`.
pub fn derives_native_error(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|s| s.ident == "NativeError")
                    })
                })
    })
}

/// The variants of a native error enum, or the compile error that explains what to change.
pub fn read(item: &ItemEnum) -> syn::Result<Vec<NativeErrorVariant>> {
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "a native error enum takes no generic parameters",
        ));
    }
    let container = parse_serde_container_attrs(&item.attrs);
    if container.tag.as_deref() != Some("code") || container.content.is_some() || container.untagged
    {
        return Err(syn::Error::new(
            item.ident.span(),
            "`NativeError` needs `#[serde(tag = \"code\")]`: each variant's name is its error code",
        ));
    }
    let rule = container.rename_all.unwrap_or(RenameRule::None);

    let mut variants = Vec::with_capacity(item.variants.len());
    for variant in &item.variants {
        let internal = native_internal(&variant.attrs)?;
        let serde = parse_serde_member_attrs(&variant.attrs);
        if internal {
            if !serde.skip {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    "an internal variant is never sent as it is; add `#[serde(skip)]`",
                ));
            }
            variants.push(NativeErrorVariant {
                ident: variant.ident.clone(),
                code: None,
            });
            continue;
        }
        if serde.skip {
            return Err(syn::Error::new(
                variant.ident.span(),
                "a skipped variant cannot be sent; mark it `#[native_error(internal)]` to send it as `E_INTERNAL`",
            ));
        }
        if let Fields::Unnamed(fields) = &variant.fields
            && fields.unnamed.len() > 1
        {
            return Err(syn::Error::new(
                variant.ident.span(),
                "serde cannot tag a tuple variant with `code`; use named fields",
            ));
        }
        let code = serde
            .rename
            .unwrap_or_else(|| rule.apply(&syn::ext::IdentExt::unraw(&variant.ident).to_string()));
        if code.starts_with("E_") {
            return Err(syn::Error::new(
                variant.ident.span(),
                format!(
                    "`{code}`: codes starting with `E_` belong to the framework; rename the variant"
                ),
            ));
        }
        variants.push(NativeErrorVariant {
            ident: variant.ident.clone(),
            code: Some(code),
        });
    }
    Ok(variants)
}

fn native_internal(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut internal = false;
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("native_error"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("internal") {
                internal = true;
                Ok(())
            } else {
                Err(meta.error("expected `#[native_error(internal)]`"))
            }
        })?;
    }
    Ok(internal)
}
