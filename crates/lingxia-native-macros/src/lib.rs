use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{
    Expr, FnArg, GenericArgument, ItemFn, Lit, LitStr, PatType, PathArguments, Token, Type,
    parse_macro_input,
};

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
    let mut blocking = false;
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
            Expr::Path(path) if path.path.is_ident("blocking") => {
                if blocking {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate blocking flag in #[{macro_name}(...)]"),
                    ));
                }
                blocking = true;
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
                        "expected a mode flag (`blocking`, `stream`, or `channel`) or `audience = \"…\"` in #[{macro_name}(...)]"
                    ),
                ));
            }
        }
    }

    if blocking && !matches!(mode, HostMode::Unary) {
        return Err(syn::Error::new_spanned(
            route_lit,
            format!("blocking is only supported for unary #[{macro_name}] handlers"),
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
            blocking,
            audience,
        },
    ))
}

#[derive(Clone, Copy)]
enum HostMode {
    Unary,
    Stream,
    Channel,
}

#[derive(Clone, Copy)]
enum AudienceRequirement {
    Optional,
    Required,
}

// The `Only` suffix makes the security audience constraints explicit at call sites.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteAudience {
    AppSessionOnly,
    AuthenticatedReadOnly,
    ControlAppOnly,
    BrowserControlOnly,
    ControlOnly,
}

impl RouteAudience {
    fn parse(value: &LitStr, macro_name: &str) -> syn::Result<Self> {
        match value.value().as_str() {
            "app-session-only" => Ok(Self::AppSessionOnly),
            "authenticated-read-only" => Ok(Self::AuthenticatedReadOnly),
            "control-app-only" => Ok(Self::ControlAppOnly),
            "browser-control-only" => Ok(Self::BrowserControlOnly),
            "control-only" => Ok(Self::ControlOnly),
            _ => Err(syn::Error::new_spanned(
                value,
                format!(
                    "unknown audience `{}` in #[{macro_name}(...)]; expected one of `app-session-only`, `authenticated-read-only`, `control-app-only`, `browser-control-only`, or `control-only`",
                    value.value()
                ),
            )),
        }
    }

    fn tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::AppSessionOnly => quote!(::lingxia::host::RouteAudience::AppSessionOnly),
            Self::AuthenticatedReadOnly => {
                quote!(::lingxia::host::RouteAudience::AuthenticatedReadOnly)
            }
            Self::ControlAppOnly => quote!(::lingxia::host::RouteAudience::ControlAppOnly),
            Self::BrowserControlOnly => {
                quote!(::lingxia::host::RouteAudience::BrowserControlOnly)
            }
            Self::ControlOnly => quote!(::lingxia::host::RouteAudience::ControlOnly),
        }
    }
}

#[derive(Clone, Copy)]
struct HostOptions {
    mode: HostMode,
    blocking: bool,
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

    let call_plan = match HostFnPlan::from_fn(&input_fn) {
        Ok(plan) => plan,
        Err(err) => return err.to_compile_error(),
    };

    let is_async = input_fn.sig.asyncness.is_some();
    if options.blocking && is_async {
        return syn::Error::new_spanned(
            input_fn.sig.asyncness,
            "#[native(..., blocking)] is only supported on non-async functions",
        )
        .to_compile_error();
    }

    let call_expr = call_plan.call_expr(&fn_ident, is_async, options.blocking);
    let ctor_ident = match options.mode {
        HostMode::Unary => format_ident!("new"),
        HostMode::Stream => format_ident!("stream"),
        HostMode::Channel => unreachable!("channel mode is handled by expand_channel"),
    };
    let serialize_expr = match options.mode {
        HostMode::Unary => quote! {
            ::lingxia::host::serialize_result(__lingxia_result)
        },
        HostMode::Stream => unreachable!("stream mode is handled by expand_stream"),
        HostMode::Channel => unreachable!("channel mode is handled by expand_channel"),
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
                    let __lingxia_result = {
                        let __lingxia_result = #call_expr;
                        __lingxia_result.map_err(::std::convert::Into::into)
                    };
                    #serialize_expr
                })
            }
        }

        #[doc(hidden)]
        pub fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
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

#[derive(Clone, Copy)]
enum HostAuthorityArg {
    None,
    LxApp,
    Invocation,
}

impl HostAuthorityArg {
    fn tokens(self) -> Option<proc_macro2::TokenStream> {
        match self {
            Self::None => None,
            Self::LxApp => Some(quote! { __lingxia_invocation.lxapp() }),
            Self::Invocation => Some(quote! { __lingxia_invocation }),
        }
    }
}

struct HostFnPlan {
    authority: HostAuthorityArg,
    input_ty: Option<Type>,
    has_cancel: bool,
}

impl HostFnPlan {
    fn from_fn(input_fn: &ItemFn) -> syn::Result<Self> {
        let mut authority = HostAuthorityArg::None;
        let mut input_ty = None;
        let mut has_cancel = false;
        let input_count = input_fn.sig.inputs.len();

        for (index, arg) in input_fn.sig.inputs.iter().enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native] does not support methods with a receiver",
                ));
            };

            if index == 0 {
                if is_lxapp_arg(arg) {
                    authority = HostAuthorityArg::LxApp;
                    continue;
                }
                if is_host_invocation_context_arg(arg) {
                    authority = HostAuthorityArg::Invocation;
                    continue;
                }
            }

            if is_host_cancel_arg(arg) {
                if index + 1 != input_count {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "HostCancel must be the last argument in a #[native] function",
                    ));
                }
                if has_cancel {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "#[native] functions can only take one HostCancel argument",
                    ));
                }
                has_cancel = true;
                continue;
            }

            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            authority,
            input_ty,
            has_cancel,
        })
    }

    fn call_expr(
        &self,
        fn_ident: &syn::Ident,
        is_async: bool,
        blocking: bool,
    ) -> proc_macro2::TokenStream {
        let mut args = Vec::new();
        let mut prelude = Vec::new();

        if let Some(authority) = self.authority.tokens() {
            args.push(authority);
        }

        if let Some(input_ty) = &self.input_ty {
            prelude.push(quote! {
                let __lingxia_payload: #input_ty =
                    ::lingxia::host::parse_input(__lingxia_input.as_deref())?;
            });
            args.push(quote! { __lingxia_payload });
        }

        if self.has_cancel {
            args.push(quote! { __lingxia_cancel });
        }

        let invoke = if is_async {
            quote! { #fn_ident(#(#args),*).await }
        } else if blocking {
            quote! {
                ::lingxia::host::__native::spawn_blocking(move || #fn_ident(#(#args),*)).await?
            }
        } else {
            quote! { #fn_ident(#(#args),*) }
        };

        quote! {
            {
                #(#prelude)*
                #invoke
            }
        }
    }
}

fn is_lxapp_arg(arg: &PatType) -> bool {
    type_is_arc_lxapp(&arg.ty)
}

fn is_host_invocation_context_arg(arg: &PatType) -> bool {
    type_is_host_invocation_context(&arg.ty)
}

fn is_host_cancel_arg(arg: &PatType) -> bool {
    type_is_host_cancel(&arg.ty)
}

fn type_is_arc_lxapp(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(last_segment) = type_path.path.segments.last() else {
        return false;
    };
    if last_segment.ident != "Arc" {
        return false;
    }
    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return false;
    };
    let Some(GenericArgument::Type(inner_ty)) = args.args.first() else {
        return false;
    };
    type_is_lxapp(inner_ty)
}

fn type_is_lxapp(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "LxApp")
        .unwrap_or(false)
}

fn type_is_host_cancel(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "HostCancel")
        .unwrap_or(false)
}

fn type_is_host_invocation_context(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "HostInvocationContext")
        .unwrap_or(false)
}

fn type_is_stream_context(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "StreamContext")
        .unwrap_or(false)
}

fn type_is_channel_context(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident == "ChannelContext")
        .unwrap_or(false)
}

fn context_type_args(ty: &Type, expected_ident: &str) -> syn::Result<Vec<Type>> {
    let Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            format!("expected `{expected_ident}`"),
        ));
    };
    let Some(last_segment) = type_path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            ty,
            format!("expected `{expected_ident}`"),
        ));
    };
    if last_segment.ident != expected_ident {
        return Err(syn::Error::new_spanned(
            ty,
            format!("expected `{expected_ident}`"),
        ));
    }

    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for arg in &args.args {
        let GenericArgument::Type(ty) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                format!("`{expected_ident}` only supports type generic arguments"),
            ));
        };
        out.push(ty.clone());
    }
    Ok(out)
}

fn parse_stream_context_types(ty: &Type) -> syn::Result<(Type, Type)> {
    let args = context_type_args(ty, "StreamContext")?;
    Ok(match args.len() {
        0 => (
            syn::parse_quote!(::lingxia::host::JsonValue),
            syn::parse_quote!(()),
        ),
        1 => (args[0].clone(), syn::parse_quote!(())),
        2 => (args[0].clone(), args[1].clone()),
        _ => {
            return Err(syn::Error::new_spanned(
                ty,
                "`StreamContext` supports at most two generic arguments",
            ));
        }
    })
}

fn parse_channel_context_types(ty: &Type) -> syn::Result<(Type, Type)> {
    let args = context_type_args(ty, "ChannelContext")?;
    Ok(match args.len() {
        0 => (
            syn::parse_quote!(::lingxia::host::JsonValue),
            syn::parse_quote!(::lingxia::host::JsonValue),
        ),
        1 => (args[0].clone(), args[0].clone()),
        2 => (args[0].clone(), args[1].clone()),
        _ => {
            return Err(syn::Error::new_spanned(
                ty,
                "`ChannelContext` supports at most two generic arguments",
            ));
        }
    })
}

// ===== Stream expansion =====

struct StreamFnPlan {
    authority: HostAuthorityArg,
    input_ty: Option<Type>,
    event_ty: Type,
    result_ty: Type,
}

impl StreamFnPlan {
    fn from_fn(input_fn: &ItemFn) -> syn::Result<Self> {
        let inputs = &input_fn.sig.inputs;

        let Some(last) = inputs.last() else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[native(..., stream)] function must take `StreamContext` as its last argument",
            ));
        };
        let FnArg::Typed(last_arg) = last else {
            return Err(syn::Error::new_spanned(
                last,
                "#[native] does not support methods with a receiver",
            ));
        };
        if !type_is_stream_context(&last_arg.ty) {
            return Err(syn::Error::new_spanned(
                last,
                "last argument of a #[native(..., stream)] function must be `StreamContext`",
            ));
        }

        let (event_ty, result_ty) = parse_stream_context_types(&last_arg.ty)?;
        let mut authority = HostAuthorityArg::None;
        let mut input_ty = None;
        let prefix_count = inputs.len() - 1;

        for (index, arg) in inputs.iter().take(prefix_count).enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native] does not support methods with a receiver",
                ));
            };
            if index == 0 {
                if is_lxapp_arg(arg) {
                    authority = HostAuthorityArg::LxApp;
                    continue;
                }
                if is_host_invocation_context_arg(arg) {
                    authority = HostAuthorityArg::Invocation;
                    continue;
                }
            }
            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native(stream)] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            authority,
            input_ty,
            event_ty,
            result_ty,
        })
    }

    fn call_expr(&self, fn_ident: &syn::Ident, is_async: bool) -> proc_macro2::TokenStream {
        let mut args: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut prelude: Vec<proc_macro2::TokenStream> = Vec::new();

        if let Some(authority) = self.authority.tokens() {
            args.push(authority);
        }

        if let Some(input_ty) = &self.input_ty {
            prelude.push(quote! {
                let __lingxia_payload: #input_ty =
                    ::lingxia::host::parse_input(__lingxia_input.as_deref())?;
            });
            args.push(quote! { __lingxia_payload });
        }

        args.push(quote! { __lingxia_stream });

        let invoke = if is_async {
            quote! { #fn_ident(#(#args),*).await }
        } else {
            quote! { #fn_ident(#(#args),*) }
        };

        quote! {
            {
                #(#prelude)*
                #invoke
            }
        }
    }
}

fn expand_stream(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    options: HostOptions,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaStreamHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());
    let audience = options.audience.tokens();

    let plan = match StreamFnPlan::from_fn(&input_fn) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };
    let call_expr = plan.call_expr(&fn_ident, input_fn.sig.asyncness.is_some());
    let event_ty = &plan.event_ty;
    let result_ty = &plan.result_ty;

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
                    let (__lingxia_stream, __lingxia_rx) =
                        ::lingxia::host::new_stream_context::<#event_ty, #result_ty>(__lingxia_cancel);
                    let __lingxia_error_tx = __lingxia_stream.error_sender();

                    ::lingxia::host::__native::spawn(async move {
                        let __lingxia_result: ::lingxia::host::HostResult<()> = {
                            let __lingxia_invocation = __lingxia_invocation;
                            let __lingxia_input = __lingxia_input;
                            let __lingxia_stream = __lingxia_stream;
                            #call_expr
                        }
                        .map_err(::std::convert::Into::into);
                        if let Err(err) = __lingxia_result {
                            let _ = __lingxia_error_tx.send(Err(err));
                        }
                    });

                    Ok(::lingxia::host::stream_output_from_rx(__lingxia_rx))
                })
            }
        }

        #[doc(hidden)]
        pub fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
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

struct ChannelFnPlan {
    authority: HostAuthorityArg,
    input_ty: Option<Type>,
    inbound_ty: Type,
    outbound_ty: Type,
}

impl ChannelFnPlan {
    fn from_fn(input_fn: &ItemFn) -> syn::Result<Self> {
        let inputs = &input_fn.sig.inputs;

        // Last argument must be ChannelContext.
        let Some(last) = inputs.last() else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[native(..., channel)] function must take `ChannelContext` as its last argument",
            ));
        };
        let FnArg::Typed(last_arg) = last else {
            return Err(syn::Error::new_spanned(
                last,
                "#[native] does not support methods with a receiver",
            ));
        };
        if !type_is_channel_context(&last_arg.ty) {
            return Err(syn::Error::new_spanned(
                last,
                "last argument of a #[native(..., channel)] function must be `ChannelContext`",
            ));
        }

        let (inbound_ty, outbound_ty) = parse_channel_context_types(&last_arg.ty)?;

        let mut authority = HostAuthorityArg::None;
        let mut input_ty = None;
        let prefix_count = inputs.len() - 1;

        for (index, arg) in inputs.iter().take(prefix_count).enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native] does not support methods with a receiver",
                ));
            };
            if index == 0 {
                if is_lxapp_arg(arg) {
                    authority = HostAuthorityArg::LxApp;
                    continue;
                }
                if is_host_invocation_context_arg(arg) {
                    authority = HostAuthorityArg::Invocation;
                    continue;
                }
            }
            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[native(channel)] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            authority,
            input_ty,
            inbound_ty,
            outbound_ty,
        })
    }

    fn call_expr(
        &self,
        fn_ident: &syn::Ident,
        is_async: bool,
        channel_ident: &syn::Ident,
    ) -> proc_macro2::TokenStream {
        let mut args: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut prelude: Vec<proc_macro2::TokenStream> = Vec::new();

        if let Some(authority) = self.authority.tokens() {
            args.push(authority);
        }

        if let Some(input_ty) = &self.input_ty {
            prelude.push(quote! {
                let __lingxia_payload: #input_ty =
                    match ::lingxia::host::parse_input(__lingxia_input.as_deref()) {
                        Ok(v) => v,
                        Err(e) => {
                            __lingxia_close.close_with("INVALID_PARAMS", e.to_string());
                            return;
                        }
                    };
            });
            args.push(quote! { __lingxia_payload });
        }

        args.push(quote! { #channel_ident });

        if is_async {
            quote! {
                {
                    #(#prelude)*
                    #fn_ident(#(#args),*).await
                }
            }
        } else {
            quote! {
                {
                    #(#prelude)*
                    #fn_ident(#(#args),*)
                }
            }
        }
    }
}

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

    let plan = match ChannelFnPlan::from_fn(&input_fn) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };

    let channel_ident = format_ident!("__lingxia_channel");
    let call_expr = plan.call_expr(&fn_ident, input_fn.sig.asyncness.is_some(), &channel_ident);
    let inbound_ty = &plan.inbound_ty;
    let outbound_ty = &plan.outbound_ty;

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
                    let __lingxia_result: ::lingxia::host::HostResult<()> = {
                        #call_expr
                    }
                    .map_err(::std::convert::Into::into);
                    match __lingxia_result {
                        Ok(()) => __lingxia_close.close(),
                        Err(err) => __lingxia_close.close_with("HOST_ERROR", err.to_string()),
                    }
                });
            }
        }

        #[doc(hidden)]
        pub fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
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
                quote!("demo.read", audience = "authenticated-read-only"),
                RouteAudience::AuthenticatedReadOnly,
                HostMode::Unary,
            ),
            (
                quote!("demo.host", audience = "control-app-only"),
                RouteAudience::ControlAppOnly,
                HostMode::Unary,
            ),
            (
                quote!("demo.browser", stream, audience = "browser-control-only"),
                RouteAudience::BrowserControlOnly,
                HostMode::Stream,
            ),
            (
                quote!("demo.any", audience = "control-only", channel),
                RouteAudience::ControlOnly,
                HostMode::Channel,
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
    fn invocation_context_is_an_authority_argument_for_every_native_mode() {
        let unary: ItemFn = syn::parse_quote! {
            fn scoped(context: lingxia::host::HostInvocationContext, input: String) -> Result<()> {
                Ok(())
            }
        };
        let stream: ItemFn = syn::parse_quote! {
            async fn scoped_stream(
                context: lingxia::host::HostInvocationContext,
                stream: lingxia::host::StreamContext<String>,
            ) -> Result<()> {
                Ok(())
            }
        };
        let channel: ItemFn = syn::parse_quote! {
            async fn scoped_channel(
                context: lingxia::host::HostInvocationContext,
                channel: lingxia::host::ChannelContext<String>,
            ) -> Result<()> {
                Ok(())
            }
        };

        assert!(matches!(
            HostFnPlan::from_fn(&unary).expect("unary plan").authority,
            HostAuthorityArg::Invocation
        ));
        assert!(matches!(
            StreamFnPlan::from_fn(&stream)
                .expect("stream plan")
                .authority,
            HostAuthorityArg::Invocation
        ));
        assert!(matches!(
            ChannelFnPlan::from_fn(&channel)
                .expect("channel plan")
                .authority,
            HostAuthorityArg::Invocation
        ));
    }
}
