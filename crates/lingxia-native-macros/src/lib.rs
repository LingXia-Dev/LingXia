use lingxia_native_codegen::syntax::{Param, Params, Shape, classify, returns_result, watch_event};
use proc_macro::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, ItemFn, Lit, LitStr, Token, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn native(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_host_attribute(attr, item, "native", AudienceRequirement::Optional)
}

/// Framework-owned routes must make their registration audience explicit.
///
/// This is deliberately separate from [`native`]: downstream host extensions
/// keep the backwards-compatible `AppSessionOnly` default, while framework code
/// cannot accidentally inherit it.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn framework_native(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_host_attribute(
        attr,
        item,
        "framework_native",
        AudienceRequirement::Required,
    )
}

/// An app error a route returns: each variant's serde `code` tag is its code, its fields
/// arrive as `data`, and a `#[native_error(internal)]` variant is sent as `E_INTERNAL`.
#[proc_macro_derive(NativeError, attributes(native_error))]
pub fn derive_native_error(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as syn::ItemEnum);
    match expand_native_error(&item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_native_error(item: &syn::ItemEnum) -> syn::Result<proc_macro2::TokenStream> {
    let variants = lingxia_native_codegen::native_error::read(item)?;
    let ident = &item.ident;
    let internal: Vec<_> = variants
        .iter()
        .filter(|v| v.code.is_none())
        .map(|v| &v.ident)
        .collect();
    let internal_arm = (!internal.is_empty()).then(|| {
        quote! {
            if matches!(error, #( #ident::#internal { .. } )|*) {
                return ::lingxia::Error::internal(message).with_source(error);
            }
        }
    });
    Ok(quote! {
        impl ::std::convert::From<#ident> for ::lingxia::Error {
            fn from(error: #ident) -> Self {
                let message = ::std::string::ToString::to_string(&error);
                #internal_arm
                ::lingxia::Error::__app(&error, message)
            }
        }
    })
}

fn expand_host_attribute(
    attr: TokenStream,
    item: TokenStream,
    macro_name: &str,
    audience_requirement: AudienceRequirement,
) -> TokenStream {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = match parser.parse(attr) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };

    let (route_lit, options) = match parse_host_attr(args, macro_name, audience_requirement) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let route = route_lit.value();
    let Some((namespace, method)) = route.rsplit_once('.') else {
        return syn::Error::new(
            route_lit.span(),
            format!("{macro_name} route must look like \"namespace.method\""),
        )
        .to_compile_error()
        .into();
    };
    if namespace.trim().is_empty() || method.trim().is_empty() {
        return syn::Error::new(
            route_lit.span(),
            format!("{macro_name} route must contain non-empty namespace and method"),
        )
        .to_compile_error()
        .into();
    }
    if namespace == "channel" {
        return syn::Error::new(
            route_lit.span(),
            format!("{macro_name} namespace 'channel' is reserved by the JS API; choose a different namespace"),
        )
        .to_compile_error()
        .into();
    }

    let input_fn = parse_macro_input!(item as ItemFn);
    match options.mode {
        HostMode::Stream => {
            expand_stream(route_lit.clone(), namespace, method, options, input_fn).into()
        }
        HostMode::Watch => {
            expand_watch(route_lit.clone(), namespace, method, options, input_fn).into()
        }
        HostMode::Channel => {
            expand_channel(route_lit.clone(), namespace, method, options, input_fn).into()
        }
        HostMode::Unary => {
            expand_host(route_lit.clone(), namespace, method, options, input_fn).into()
        }
    }
}

fn parse_host_attr(
    args: Punctuated<Expr, Token![,]>,
    macro_name: &str,
    audience_requirement: AudienceRequirement,
) -> syn::Result<(LitStr, HostOptions)> {
    let Some(first) = args.first() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("expected #[{macro_name}(\"namespace.method\")]"),
        ));
    };
    let Expr::Lit(first_lit) = first else {
        return Err(syn::Error::new_spanned(
            first,
            format!("expected #[{macro_name}(\"namespace.method\")]"),
        ));
    };
    let Lit::Str(route_lit) = &first_lit.lit else {
        return Err(syn::Error::new_spanned(
            &first_lit.lit,
            format!("expected #[{macro_name}(\"namespace.method\")]"),
        ));
    };

    let mut mode = HostMode::Unary;
    let mut sync = false;
    let mut audience = None;
    for arg in args.iter().skip(1) {
        match arg {
            Expr::Path(path) if path.path.is_ident("stream") => {
                if !matches!(mode, HostMode::Unary) {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate or conflicting mode flag in #[{macro_name}(...)]"),
                    ));
                }
                mode = HostMode::Stream;
            }
            Expr::Path(path) if path.path.is_ident("channel") => {
                if !matches!(mode, HostMode::Unary) {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate or conflicting mode flag in #[{macro_name}(...)]"),
                    ));
                }
                mode = HostMode::Channel;
            }
            Expr::Path(path) if path.path.is_ident("watch") => {
                if !matches!(mode, HostMode::Unary) {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate or conflicting mode flag in #[{macro_name}(...)]"),
                    ));
                }
                mode = HostMode::Watch;
            }
            Expr::Path(path) if path.path.is_ident("blocking") => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "a plain `fn` route runs on a blocking thread; remove `blocking`",
                ));
            }
            Expr::Path(path) if path.path.is_ident("sync") => {
                if sync {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate sync flag in #[{macro_name}(...)]"),
                    ));
                }
                sync = true;
            }
            Expr::Assign(assign) if matches!(assign.left.as_ref(), Expr::Path(path) if path.path.is_ident("audience")) =>
            {
                if audience.is_some() {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate audience option in #[{macro_name}(...)]"),
                    ));
                }
                let Expr::Lit(value) = assign.right.as_ref() else {
                    return Err(syn::Error::new_spanned(
                        &assign.right,
                        format!("audience must be a string literal in #[{macro_name}(...)]"),
                    ));
                };
                let Lit::Str(value) = &value.lit else {
                    return Err(syn::Error::new_spanned(
                        &value.lit,
                        format!("audience must be a string literal in #[{macro_name}(...)]"),
                    ));
                };
                audience = Some(RouteAudience::parse(value, macro_name)?);
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    format!(
                        "expected a mode flag (`sync`, `stream`, `watch`, or `channel`) or `audience = \"…\"` in #[{macro_name}(...)]"
                    ),
                ));
            }
        }
    }

    if sync && !matches!(mode, HostMode::Unary) {
        return Err(syn::Error::new_spanned(
            route_lit,
            format!("sync is only supported for unary #[{macro_name}] handlers"),
        ));
    }

    let audience = match (audience, audience_requirement) {
        (Some(audience), _) => audience,
        (None, AudienceRequirement::Optional) => RouteAudience::AppSessionOnly,
        (None, AudienceRequirement::Required) => {
            return Err(syn::Error::new_spanned(
                route_lit,
                format!("#[{macro_name}] requires `audience = \"…\"`"),
            ));
        }
    };

    Ok((
        route_lit.clone(),
        HostOptions {
            mode,
            sync,
            audience,
        },
    ))
}

#[derive(Clone, Copy)]
enum HostMode {
    Unary,
    Stream,
    Watch,
    Channel,
}

#[derive(Clone, Copy)]
enum AudienceRequirement {
    Optional,
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteAudience {
    AppSessionOnly,
    ControlAppOnly,
    ControlSurfaceOnly,
    ControlAppOrSurface,
}

impl RouteAudience {
    fn parse(value: &LitStr, macro_name: &str) -> syn::Result<Self> {
        match value.value().as_str() {
            "app-session-only" => Ok(Self::AppSessionOnly),
            "control-app-only" => Ok(Self::ControlAppOnly),
            "control-surface-only" => Ok(Self::ControlSurfaceOnly),
            "control-app-or-surface" => Ok(Self::ControlAppOrSurface),
            _ => Err(syn::Error::new_spanned(
                value,
                format!(
                    "unknown audience `{}` in #[{macro_name}(...)]; expected one of `app-session-only`, `control-app-only`, `control-surface-only`, or `control-app-or-surface`",
                    value.value()
                ),
            )),
        }
    }

    fn tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::AppSessionOnly => quote!(::lingxia::host::RouteAudience::AppSessionOnly),
            Self::ControlAppOnly => quote!(::lingxia::host::RouteAudience::ControlAppOnly),
            Self::ControlSurfaceOnly => {
                quote!(::lingxia::host::RouteAudience::ControlSurfaceOnly)
            }
            Self::ControlAppOrSurface => {
                quote!(::lingxia::host::RouteAudience::ControlAppOrSurface)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct HostOptions {
    mode: HostMode,
    sync: bool,
    audience: RouteAudience,
}

fn expand_host(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaHostHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());
    let audience = options.audience.tokens();

    let shape = if options.sync {
        Shape::Sync
    } else {
        Shape::Call
    };
    let call_plan = match Plan::new(&input_fn, shape) {
        Ok(plan) => plan,
        Err(err) => return err.to_compile_error(),
    };

    let is_async = input_fn.sig.asyncness.is_some();
    if options.sync && is_async {
        return syn::Error::new_spanned(
            input_fn.sig.asyncness,
            "#[native(..., sync)] is only supported on non-async functions",
        )
        .to_compile_error();
    }

    if options.sync {
        let call_expr = call_plan.call_expr(&fn_ident, false);
        return quote! {
            #input_fn

            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            pub struct #handler_ident;

            impl ::lingxia::host::SyncHostHandler for #handler_ident {
                fn call_sync(
                    &self,
                    __lingxia_invocation: ::lingxia::host::HostInvocationContext,
                    __lingxia_input: Option<String>,
                ) -> ::lingxia::Result<::lingxia::host::HostOutput> {
                    ::lingxia::host::serialize_result(#call_expr)
                }
            }

            #[doc(hidden)]
            pub(crate) fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
                ::lingxia::host::HostRegistrationEntry::SyncHandler(
                    ::lingxia::host::SyncHostRegistration::new(
                        #namespace_lit,
                        #method_lit,
                        #audience,
                        std::sync::Arc::new(#handler_ident),
                    )
                )
            }
        };
    }

    let call_expr = call_plan.call_expr(&fn_ident, is_async);
    let ctor_ident = match options.mode {
        HostMode::Unary => format_ident!("new"),
        HostMode::Stream | HostMode::Watch | HostMode::Channel => {
            unreachable!("stream, watch and channel modes have their own expansion")
        }
    };
    let serialize_expr = match options.mode {
        HostMode::Unary => quote! {
            ::lingxia::host::serialize_result(__lingxia_result)
        },
        HostMode::Stream | HostMode::Watch | HostMode::Channel => {
            unreachable!("stream, watch and channel modes have their own expansion")
        }
    };
    quote! {
        #input_fn

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub struct #handler_ident;

        impl ::lingxia::host::HostHandler for #handler_ident {
            fn call<'a>(
                &'a self,
                __lingxia_invocation: ::lingxia::host::HostInvocationContext,
                __lingxia_input: Option<String>,
                __lingxia_cancel: ::lingxia::host::HostCancel,
            ) -> ::lingxia::host::HostFuture<'a> {
                Box::pin(async move {
                    let __lingxia_result = #call_expr;
                    #serialize_expr
                })
            }
        }

        #[doc(hidden)]
        pub(crate) fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
            ::lingxia::host::HostRegistrationEntry::Handler(
                ::lingxia::host::HostRegistration::#ctor_ident(
                    #namespace_lit,
                    #method_lit,
                    #audience,
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}

/// Call glue for one route; parameters come in any order, classified by type.
struct Plan {
    shape: Shape,
    params: Params,
    /// The fn's return type, `(T, E)` for a `Result<T, E>`; `None` when it returns `()`.
    output: Option<(Type, Option<Type>)>,
    fallible: bool,
}

impl Plan {
    fn new(input_fn: &ItemFn, shape: Shape) -> syn::Result<Self> {
        let sig = &input_fn.sig;
        let fallible = returns_result(sig);
        let output = match &sig.output {
            syn::ReturnType::Type(_, ty) if fallible => {
                let mut args = result_args(ty).into_iter();
                args.next().map(|ok| (ok, args.next()))
            }
            syn::ReturnType::Type(_, ty) => Some(((**ty).clone(), None)),
            syn::ReturnType::Default => None,
        };
        Ok(Self {
            shape,
            params: classify(sig, shape)?,
            output,
            fallible,
        })
    }

    /// `value` as a `lingxia::Result`. The turbofish repeats the user's own return types,
    /// so a missing `Serialize` or error conversion is reported at them.
    fn wrap(&self, value: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let named = |ty: &Type| -> proc_macro2::TokenStream {
            if matches!(ty, Type::ImplTrait(_)) {
                quote!(_)
            } else {
                quote!(#ty)
            }
        };
        let (ok, err) = match &self.output {
            Some((ok, err)) => (named(ok), err.as_ref().map(named).unwrap_or(quote!(_))),
            None => (quote!(()), quote!(_)),
        };
        match (self.shape, self.fallible) {
            (Shape::Watch, true) => {
                quote! { ::lingxia::host::__native::opened::<_, #err>(#value) }
            }
            (Shape::Watch, false) => {
                quote! { ::core::result::Result::Ok::<_, ::lingxia::Error>(#value) }
            }
            (_, true) => {
                // Spanned at the error type: a missing `From` for `lingxia::Error` lands there.
                let span = match &self.output {
                    Some((_, Some(err))) => err.span(),
                    _ => proc_macro2::Span::call_site(),
                };
                quote_spanned! {span=> ::lingxia::host::__native::result::<#ok, #err>(#value) }
            }
            (_, false) => quote! { ::lingxia::host::__native::ok::<#ok>(#value) },
        }
    }

    fn progress_event(&self) -> Type {
        self.params
            .progress()
            .flatten()
            .cloned()
            .unwrap_or_else(|| syn::parse_quote!(::lingxia::host::JsonValue))
    }

    fn channel_types(&self) -> (Type, Type) {
        let (inbound, outbound) = self.params.channel().unwrap_or_default();
        let json = || syn::parse_quote!(::lingxia::host::JsonValue);
        (
            inbound.cloned().unwrap_or_else(json),
            outbound.cloned().unwrap_or_else(json),
        )
    }

    /// A plain `fn` runs on the blocking pool unless it is `sync` (the Logic JS thread).
    fn call_expr(&self, fn_ident: &syn::Ident, is_async: bool) -> proc_macro2::TokenStream {
        let mut call_args = Vec::new();
        let mut prelude = Vec::new();

        for (index, param) in self.params.0.iter().enumerate() {
            match param {
                Param::LxApp => call_args.push(quote! { __lingxia_invocation.lxapp() }),
                Param::Invocation => call_args.push(quote! { __lingxia_invocation.clone() }),
                Param::Cancel => call_args.push(quote! { __lingxia_cancel }),
                Param::Progress(_) => call_args.push(quote! { __lingxia_progress }),
                Param::Channel { .. } => call_args.push(quote! { __lingxia_channel }),
                Param::Payload(input_ty) => {
                    let parse = quote_spanned! {input_ty.span()=>
                        ::lingxia::host::__native::payload::<#input_ty>(__lingxia_input.as_deref())
                    };
                    prelude.push(if self.shape == Shape::Channel {
                        quote! {
                            let __lingxia_payload: #input_ty = match #parse {
                                Ok(v) => v,
                                Err(e) => {
                                    __lingxia_close.close_with(e);
                                    return;
                                }
                            };
                        }
                    } else {
                        quote! { let __lingxia_payload: #input_ty = #parse?; }
                    });
                    call_args.push(quote! { __lingxia_payload });
                }
                Param::PageState(state_ty) => {
                    let ident = format_ident!("__lingxia_page_state_{index}");
                    prelude.push(if self.shape == Shape::Sync {
                        quote! {
                            let #ident: #state_ty =
                                <#state_ty>::acquire_sync(&__lingxia_invocation)?;
                        }
                    } else {
                        quote! {
                            let #ident: #state_ty =
                                <#state_ty>::acquire_async(&__lingxia_invocation).await?;
                        }
                    });
                    call_args.push(quote! { #ident });
                }
            }
        }

        let call = quote! { #fn_ident(#(#call_args),*) };
        let invoke = if is_async {
            self.wrap(quote! { #call.await })
        } else if self.shape == Shape::Sync {
            self.wrap(call)
        } else {
            let wrapped = self.wrap(call);
            quote! { ::lingxia::host::__native::call_blocking(move || #wrapped).await }
        };

        quote! {
            {
                #(#prelude)*
                #invoke
            }
        }
    }
}

// ===== Stream and watch expansion =====

fn expand_stream(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let plan = match Plan::new(&input_fn, Shape::Stream) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };
    let call_expr = plan.call_expr(&input_fn.sig.ident, input_fn.sig.asyncness.is_some());
    let event_ty = plan.progress_event();
    let body = quote! {
        Ok(::lingxia::host::__stream::run::<#event_ty, _, _, _>(
            __lingxia_cancel,
            move |__lingxia_progress| async move { #call_expr },
        ))
    };
    stream_route(route_lit, namespace, method, options, input_fn, body)
}

fn expand_watch(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let plan = match Plan::new(&input_fn, Shape::Watch) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };
    if let Err(err) = watch_event(&input_fn.sig) {
        return err.to_compile_error();
    }
    let call_expr = plan.call_expr(&input_fn.sig.ident, input_fn.sig.asyncness.is_some());
    let body = quote! {
        let __lingxia_events = #call_expr?;
        Ok(::lingxia::host::__stream::watch(__lingxia_events, __lingxia_cancel))
    };
    stream_route(route_lit, namespace, method, options, input_fn, body)
}

/// A route on the stream wire: `body` builds its `HostOutput`.
fn stream_route(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let fn_ident = &input_fn.sig.ident;
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaStreamHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());
    let audience = options.audience.tokens();

    quote! {
        #input_fn

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub struct #handler_ident;

        impl ::lingxia::host::HostHandler for #handler_ident {
            fn call<'a>(
                &'a self,
                __lingxia_invocation: ::lingxia::host::HostInvocationContext,
                __lingxia_input: Option<String>,
                __lingxia_cancel: ::lingxia::host::HostCancel,
            ) -> ::lingxia::host::HostFuture<'a> {
                Box::pin(async move { #body })
            }
        }

        #[doc(hidden)]
        pub(crate) fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
            ::lingxia::host::HostRegistrationEntry::Handler(
                ::lingxia::host::HostRegistration::stream(
                    #namespace_lit,
                    #method_lit,
                    #audience,
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}

// ===== Channel expansion =====

fn expand_channel(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaChannelHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());
    let audience = options.audience.tokens();

    let plan = match Plan::new(&input_fn, Shape::Channel) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };

    let channel_ident = format_ident!("__lingxia_channel");
    let call_expr = plan.call_expr(&fn_ident, input_fn.sig.asyncness.is_some());
    let (inbound_ty, outbound_ty) = plan.channel_types();

    quote! {
        #input_fn

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        pub struct #handler_ident;

        impl ::lingxia::host::ChannelHandler for #handler_ident {
            #[allow(unused_variables)]
            fn on_open(
                &self,
                __lingxia_invocation: ::lingxia::host::HostInvocationContext,
                __lingxia_ctx: ::lingxia::host::ChannelContext,
                __lingxia_input: Option<String>,
            ) {
                ::lingxia::host::__native::spawn(async move {
                    let mut __lingxia_ctx = __lingxia_ctx;
                    let __lingxia_close = __lingxia_ctx.close_handle();
                    __lingxia_ctx.disable_close_on_drop();
                    let #channel_ident =
                        __lingxia_ctx.with_types::<#inbound_ty, #outbound_ty>();
                    let __lingxia_result: ::lingxia::Result<()> = #call_expr;
                    match __lingxia_result {
                        Ok(()) => __lingxia_close.close(),
                        Err(err) => __lingxia_close.close_with(err),
                    }
                });
            }
        }

        #[doc(hidden)]
        pub(crate) fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
            ::lingxia::host::HostRegistrationEntry::Channel(
                ::lingxia::host::ChannelRegistration::new(
                    #namespace_lit,
                    #method_lit,
                    #audience,
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}

/// The generic arguments of a `Result<T, E>` (or a one-argument alias).
fn result_args(ty: &Type) -> Vec<Type> {
    let Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(syn::PathArguments::AngleBracketed(args)) =
        path.path.segments.last().map(|segment| &segment.arguments)
    else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_args(tokens: proc_macro2::TokenStream) -> Punctuated<Expr, Token![,]> {
        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        parser.parse2(tokens).expect("parse native attribute args")
    }

    #[test]
    fn native_defaults_audience_to_app_session_only() {
        let (_, options) = parse_host_attr(
            parse_args(quote!("demo.load")),
            "native",
            AudienceRequirement::Optional,
        )
        .expect("default audience");

        assert_eq!(options.audience, RouteAudience::AppSessionOnly);
        assert_eq!(
            options.audience.tokens().to_string(),
            ":: lingxia :: host :: RouteAudience :: AppSessionOnly"
        );
    }

    #[test]
    fn parses_all_supported_audiences_with_modes() {
        let cases = [
            (
                quote!("demo.app", audience = "app-session-only"),
                RouteAudience::AppSessionOnly,
                HostMode::Unary,
            ),
            (
                quote!("demo.host", audience = "control-app-only"),
                RouteAudience::ControlAppOnly,
                HostMode::Unary,
            ),
            (
                quote!("demo.surface", stream, audience = "control-surface-only"),
                RouteAudience::ControlSurfaceOnly,
                HostMode::Stream,
            ),
            (
                quote!("demo.any", audience = "control-app-or-surface", channel),
                RouteAudience::ControlAppOrSurface,
                HostMode::Channel,
            ),
            (
                quote!("demo.watch", watch, audience = "control-surface-only"),
                RouteAudience::ControlSurfaceOnly,
                HostMode::Watch,
            ),
        ];

        for (args, audience, mode) in cases {
            let (_, options) =
                parse_host_attr(parse_args(args), "native", AudienceRequirement::Optional)
                    .expect("supported audience");
            assert_eq!(options.audience, audience);
            assert!(matches!(
                (options.mode, mode),
                (HostMode::Unary, HostMode::Unary)
                    | (HostMode::Stream, HostMode::Stream)
                    | (HostMode::Watch, HostMode::Watch)
                    | (HostMode::Channel, HostMode::Channel)
            ));
        }
    }

    #[test]
    fn rejects_invalid_or_duplicate_audience() {
        for args in [
            quote!("demo.invalid", audience = "guest"),
            quote!("demo.nonliteral", audience = DEFAULT_AUDIENCE),
            quote!(
                "demo.duplicate",
                audience = "app-session-only",
                audience = "control-app-only"
            ),
        ] {
            assert!(
                parse_host_attr(parse_args(args), "native", AudienceRequirement::Optional).is_err()
            );
        }
    }

    #[test]
    fn framework_native_requires_an_explicit_audience() {
        let result = parse_host_attr(
            parse_args(quote!("framework.route")),
            "framework_native",
            AudienceRequirement::Required,
        );
        let error = match result {
            Ok(_) => panic!("framework routes require an audience"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("requires `audience = \"…\"`"));
    }

    #[test]
    fn extractors_bind_in_declaration_order_for_every_mode() {
        let unary: ItemFn = syn::parse_quote! {
            fn scoped(input: String, cancel: HostCancel, context: HostInvocationContext) -> Result<()> {
                Ok(())
            }
        };
        let stream: ItemFn = syn::parse_quote! {
            async fn scoped_stream(
                progress: lingxia::Progress<String>,
                input: Query,
                context: lingxia::host::HostInvocationContext,
            ) -> Result<()> {
                Ok(())
            }
        };
        let channel: ItemFn = syn::parse_quote! {
            async fn scoped_channel(
                channel: lingxia::host::ChannelContext<String>,
                app: Arc<LxApp>,
            ) -> Result<()> {
                Ok(())
            }
        };

        let call = |item: &ItemFn, shape| {
            Plan::new(item, shape)
                .expect("plan")
                .call_expr(&item.sig.ident, true)
                .to_string()
                .replace(' ', "")
        };
        assert!(
            call(&unary, Shape::Call).contains(
                "scoped(__lingxia_payload,__lingxia_cancel,__lingxia_invocation.clone())"
            )
        );
        assert!(call(&stream, Shape::Stream).contains(
            "scoped_stream(__lingxia_progress,__lingxia_payload,__lingxia_invocation.clone())"
        ));
        assert!(
            call(&channel, Shape::Channel)
                .contains("scoped_channel(__lingxia_channel,__lingxia_invocation.lxapp())")
        );
    }

    #[test]
    fn a_second_payload_fails_the_expansion() {
        let item: ItemFn = syn::parse_quote! {
            fn two(a: Query, b: Filter) -> Result<()> { Ok(()) }
        };
        let options = HostOptions {
            mode: HostMode::Unary,
            sync: false,
            audience: RouteAudience::AppSessionOnly,
        };
        let out = expand_host(
            LitStr::new("demo.two", proc_macro2::Span::call_site()),
            "demo",
            "two",
            options,
            item,
        )
        .to_string();
        assert!(out.contains("compile_error"), "{out}");
        assert!(out.contains("one payload argument"), "{out}");
    }

    fn expand(args: proc_macro2::TokenStream, item: ItemFn) -> String {
        let (route, options) =
            parse_host_attr(parse_args(args), "native", AudienceRequirement::Optional)
                .expect("route attribute");
        let (namespace, method) = route
            .value()
            .rsplit_once('.')
            .map(|(n, m)| (n.to_string(), m.to_string()))
            .unwrap();
        match options.mode {
            HostMode::Stream => expand_stream(route, &namespace, &method, options, item),
            HostMode::Watch => expand_watch(route, &namespace, &method, options, item),
            HostMode::Channel => expand_channel(route, &namespace, &method, options, item),
            HostMode::Unary => expand_host(route, &namespace, &method, options, item),
        }
        .to_string()
        .replace(' ', "")
    }

    #[test]
    fn a_plain_fn_runs_on_the_blocking_pool_and_sync_on_the_caller() {
        let plain: ItemFn = syn::parse_quote! {
            fn load(input: Query) -> lingxia::Result<String> { Ok(String::new()) }
        };
        let out = expand(quote!("demo.load"), plain.clone());
        assert!(
            out.contains(
                "::lingxia::host::__native::call_blocking(move||::lingxia::host::__native::result::<String,_>(load(__lingxia_payload))).await"
            ),
            "{out}"
        );
        assert!(out.contains("impl::lingxia::host::HostHandlerfor"), "{out}");

        let out = expand(quote!("demo.load", sync), plain);
        assert!(!out.contains("call_blocking"), "{out}");
        assert!(
            out.contains("impl::lingxia::host::SyncHostHandlerfor"),
            "{out}"
        );
        assert!(out.contains("load(__lingxia_payload)"), "{out}");

        let asynchronous: ItemFn = syn::parse_quote! {
            async fn fetch() -> lingxia::Result<()> { Ok(()) }
        };
        let out = expand(quote!("demo.fetch"), asynchronous);
        assert!(!out.contains("call_blocking"), "{out}");
        assert!(out.contains("fetch().await"), "{out}");

        let stream: ItemFn = syn::parse_quote! {
            fn export(progress: lingxia::Progress<u32>) -> lingxia::Result<String> { Ok(String::new()) }
        };
        let out = expand(quote!("demo.export", stream), stream);
        assert!(
            out.contains("call_blocking(move||::lingxia::host::__native::result::<String,_>(export(__lingxia_progress))).await"),
            "{out}"
        );
    }

    #[test]
    fn a_stream_returns_its_result_and_a_watch_returns_its_events() {
        let stream: ItemFn = syn::parse_quote! {
            async fn import(input: Query, progress: lingxia::Progress<Step>) -> Result<Summary, NotesError> {
                todo!()
            }
        };
        let out = expand(quote!("notes.import", stream), stream);
        assert!(
            out.contains("::lingxia::host::__stream::run::<Step,_,_,_>(__lingxia_cancel"),
            "{out}"
        );
        assert!(
            out.contains("import(__lingxia_payload,__lingxia_progress).await"),
            "{out}"
        );

        let watch: ItemFn = syn::parse_quote! {
            async fn follow(app: Arc<LxApp>) -> lingxia::Result<impl Stream<Item = Change> + Send + 'static> {
                todo!()
            }
        };
        let out = expand(quote!("notes.watch", watch), watch);
        assert!(
            out.contains("::lingxia::host::__stream::watch(__lingxia_events,__lingxia_cancel)"),
            "{out}"
        );
        assert!(out.contains("HostRegistration::stream("), "{out}");

        let not_a_stream: ItemFn = syn::parse_quote! {
            async fn follow() -> lingxia::Result<Vec<Change>> { todo!() }
        };
        let out = expand(quote!("notes.watch", watch), not_a_stream);
        assert!(out.contains("implStream<Item=Event>"), "{out}");

        let with_progress: ItemFn = syn::parse_quote! {
            async fn follow(progress: lingxia::Progress<Change>) -> lingxia::Result<impl Stream<Item = Change>> {
                todo!()
            }
        };
        let out = expand(quote!("notes.watch", watch), with_progress);
        assert!(out.contains("needs`#[native(...,stream)]`"), "{out}");
    }

    #[test]
    fn the_blocking_flag_is_rejected_with_the_fix() {
        let error = match parse_host_attr(
            parse_args(quote!("demo.load", blocking)),
            "native",
            AudienceRequirement::Optional,
        ) {
            Ok(_) => panic!("`blocking` is not a flag"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("remove `blocking`"), "{error}");
    }

    #[test]
    fn sync_flag_rejects_async_and_supports_page_state() {
        let sync_fn: ItemFn = syn::parse_quote! {
            fn bump(mut state: lingxia::host::PageState<Counter>, delta: i32) -> Result<i32> {
                Ok(delta)
            }
        };
        let plan = Plan::new(&sync_fn, Shape::Sync).expect("sync plan with PageState");
        let tokens = plan.call_expr(&sync_fn.sig.ident, false).to_string();
        assert!(tokens.contains("acquire_sync"), "{tokens}");
        assert!(tokens.contains("payload"), "{tokens}");

        let async_fn: ItemFn = syn::parse_quote! {
            async fn bad_sync() -> Result<()> { Ok(()) }
        };
        let options = HostOptions {
            mode: HostMode::Unary,
            sync: true,
            audience: RouteAudience::AppSessionOnly,
        };
        let out = expand_host(
            LitStr::new("demo.bad", proc_macro2::Span::call_site()),
            "demo",
            "bad",
            options,
            async_fn,
        )
        .to_string();
        assert!(
            out.contains("only supported on non-async functions"),
            "{out}"
        );
    }

    #[test]
    fn a_plain_return_value_and_the_payload_are_checked_at_the_users_types() {
        let infallible: ItemFn = syn::parse_quote! {
            fn ping(input: Query) -> String { String::new() }
        };
        let out = expand(quote!("demo.ping"), infallible);
        assert!(
            out.contains("call_blocking(move||::lingxia::host::__native::ok::<String>(ping(__lingxia_payload))).await"),
            "{out}"
        );
        assert!(
            out.contains("::lingxia::host::__native::payload::<Query>(__lingxia_input.as_deref())"),
            "{out}"
        );

        let unit: ItemFn = syn::parse_quote! {
            async fn touch() {}
        };
        let out = expand(quote!("demo.touch"), unit);
        assert!(
            out.contains("::lingxia::host::__native::ok::<()>(touch().await)"),
            "{out}"
        );

        let watch: ItemFn = syn::parse_quote! {
            fn follow() -> impl Stream<Item = Change> + Send + 'static { todo!() }
        };
        let out = expand(quote!("demo.watch", watch), watch);
        assert!(
            out.contains("::core::result::Result::Ok::<_,::lingxia::Error>(follow())"),
            "{out}"
        );
    }
}
