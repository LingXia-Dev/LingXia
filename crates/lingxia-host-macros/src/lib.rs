use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{
    Expr, FnArg, GenericArgument, ItemFn, Lit, LitStr, PatType, PathArguments, Token, Type,
    parse_macro_input,
};

#[proc_macro_attribute]
pub fn host(attr: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = match parser.parse(attr) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error().into(),
    };

    let (route_lit, mode) = match parse_host_attr(args) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let route = route_lit.value();
    let Some((namespace, method)) = route.rsplit_once('.') else {
        return syn::Error::new(
            route_lit.span(),
            "host route must look like \"namespace.method\"",
        )
        .to_compile_error()
        .into();
    };
    if namespace.trim().is_empty() || method.trim().is_empty() {
        return syn::Error::new(
            route_lit.span(),
            "host route must contain non-empty namespace and method",
        )
        .to_compile_error()
        .into();
    }
    if namespace == "channel" {
        return syn::Error::new(
            route_lit.span(),
            "host namespace 'channel' is reserved by the JS API; choose a different namespace",
        )
        .to_compile_error()
        .into();
    }

    let input_fn = parse_macro_input!(item as ItemFn);
    match mode {
        HostMode::Stream => expand_stream(route_lit.clone(), namespace, method, input_fn).into(),
        HostMode::Channel => expand_channel(route_lit.clone(), namespace, method, input_fn).into(),
        HostMode::Unary => expand_host(route_lit.clone(), namespace, method, mode, input_fn).into(),
    }
}

fn parse_host_attr(args: Punctuated<Expr, Token![,]>) -> syn::Result<(LitStr, HostMode)> {
    let Some(first) = args.first() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected #[host(\"namespace.method\")]",
        ));
    };
    let Expr::Lit(first_lit) = first else {
        return Err(syn::Error::new_spanned(
            first,
            "expected #[host(\"namespace.method\")]",
        ));
    };
    let Lit::Str(route_lit) = &first_lit.lit else {
        return Err(syn::Error::new_spanned(
            &first_lit.lit,
            "expected #[host(\"namespace.method\")]",
        ));
    };

    let mut mode = HostMode::Unary;
    for arg in args.iter().skip(1) {
        match arg {
            Expr::Path(path) if path.path.is_ident("stream") => {
                if !matches!(mode, HostMode::Unary) {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "duplicate or conflicting mode flag in #[host(...)]",
                    ));
                }
                mode = HostMode::Stream;
            }
            Expr::Path(path) if path.path.is_ident("channel") => {
                if !matches!(mode, HostMode::Unary) {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "duplicate or conflicting mode flag in #[host(...)]",
                    ));
                }
                mode = HostMode::Channel;
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "expected only #[host(\"namespace.method\")], #[host(\"namespace.method\", stream)], or #[host(\"namespace.method\", channel)]",
                ));
            }
        }
    }

    Ok((route_lit.clone(), mode))
}

#[derive(Clone, Copy)]
enum HostMode {
    Unary,
    Stream,
    Channel,
}

fn expand_host(
    route_lit: LitStr,
    namespace: &str,
    method: &str,
    mode: HostMode,
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaHostHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());

    let call_plan = match HostFnPlan::from_fn(&input_fn) {
        Ok(plan) => plan,
        Err(err) => return err.to_compile_error(),
    };

    let call_expr = call_plan.call_expr(&fn_ident, input_fn.sig.asyncness.is_some());
    let ctor_ident = match mode {
        HostMode::Unary => format_ident!("new"),
        HostMode::Stream => format_ident!("stream"),
        HostMode::Channel => unreachable!("channel mode is handled by expand_channel"),
    };
    let serialize_expr = match mode {
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
                __lingxia_lxapp: std::sync::Arc<::lingxia::LxApp>,
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
        pub fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
            ::lingxia::host::HostRegistrationEntry::Handler(
                ::lingxia::host::HostRegistration::#ctor_ident(
                    #namespace_lit,
                    #method_lit,
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}

struct HostFnPlan {
    has_lxapp: bool,
    input_ty: Option<Type>,
    has_cancel: bool,
}

impl HostFnPlan {
    fn from_fn(input_fn: &ItemFn) -> syn::Result<Self> {
        let mut has_lxapp = false;
        let mut input_ty = None;
        let mut has_cancel = false;
        let input_count = input_fn.sig.inputs.len();

        for (index, arg) in input_fn.sig.inputs.iter().enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host] does not support methods with a receiver",
                ));
            };

            if index == 0 && is_lxapp_arg(arg) {
                has_lxapp = true;
                continue;
            }

            if is_host_cancel_arg(arg) {
                if index + 1 != input_count {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "HostCancel must be the last argument in a #[host] function",
                    ));
                }
                if has_cancel {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "#[host] functions can only take one HostCancel argument",
                    ));
                }
                has_cancel = true;
                continue;
            }

            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            has_lxapp,
            input_ty,
            has_cancel,
        })
    }

    fn call_expr(&self, fn_ident: &syn::Ident, is_async: bool) -> proc_macro2::TokenStream {
        let mut args = Vec::new();
        let mut prelude = Vec::new();

        if self.has_lxapp {
            args.push(quote! { __lingxia_lxapp });
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
    has_lxapp: bool,
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
                "#[host(..., stream)] function must take `StreamContext` as its last argument",
            ));
        };
        let FnArg::Typed(last_arg) = last else {
            return Err(syn::Error::new_spanned(
                last,
                "#[host] does not support methods with a receiver",
            ));
        };
        if !type_is_stream_context(&last_arg.ty) {
            return Err(syn::Error::new_spanned(
                last,
                "last argument of a #[host(..., stream)] function must be `StreamContext`",
            ));
        }

        let (event_ty, result_ty) = parse_stream_context_types(&last_arg.ty)?;
        let mut has_lxapp = false;
        let mut input_ty = None;
        let prefix_count = inputs.len() - 1;

        for (index, arg) in inputs.iter().take(prefix_count).enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host] does not support methods with a receiver",
                ));
            };
            if index == 0 && is_lxapp_arg(arg) {
                has_lxapp = true;
                continue;
            }
            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host(stream)] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            has_lxapp,
            input_ty,
            event_ty,
            result_ty,
        })
    }

    fn call_expr(&self, fn_ident: &syn::Ident, is_async: bool) -> proc_macro2::TokenStream {
        let mut args: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut prelude: Vec<proc_macro2::TokenStream> = Vec::new();

        if self.has_lxapp {
            args.push(quote! { __lingxia_lxapp });
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
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaStreamHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());

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
                __lingxia_lxapp: std::sync::Arc<::lingxia::LxApp>,
                __lingxia_input: Option<String>,
                __lingxia_cancel: ::lingxia::host::HostCancel,
            ) -> ::lingxia::host::HostFuture<'a> {
                Box::pin(async move {
                    let (__lingxia_stream, __lingxia_rx) =
                        ::lingxia::host::new_stream_context::<#event_ty, #result_ty>(__lingxia_cancel);
                    let __lingxia_error_tx = __lingxia_stream.error_sender();

                    ::lingxia::tokio::task::spawn(async move {
                        let __lingxia_result: ::lingxia::host::HostResult<()> = {
                            let __lingxia_lxapp = __lingxia_lxapp;
                            let __lingxia_input = __lingxia_input;
                            let __lingxia_stream = __lingxia_stream;
                            #call_expr
                        };
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
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}

// ===== Channel expansion =====

struct ChannelFnPlan {
    has_lxapp: bool,
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
                "#[host(..., channel)] function must take `ChannelContext` as its last argument",
            ));
        };
        let FnArg::Typed(last_arg) = last else {
            return Err(syn::Error::new_spanned(
                last,
                "#[host] does not support methods with a receiver",
            ));
        };
        if !type_is_channel_context(&last_arg.ty) {
            return Err(syn::Error::new_spanned(
                last,
                "last argument of a #[host(..., channel)] function must be `ChannelContext`",
            ));
        }

        let (inbound_ty, outbound_ty) = parse_channel_context_types(&last_arg.ty)?;

        let mut has_lxapp = false;
        let mut input_ty = None;
        let prefix_count = inputs.len() - 1;

        for (index, arg) in inputs.iter().take(prefix_count).enumerate() {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host] does not support methods with a receiver",
                ));
            };
            if index == 0 && is_lxapp_arg(arg) {
                has_lxapp = true;
                continue;
            }
            if input_ty.is_some() {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[host(channel)] functions support at most one JSON payload argument",
                ));
            }
            input_ty = Some((*arg.ty).clone());
        }

        Ok(Self {
            has_lxapp,
            input_ty,
            inbound_ty,
            outbound_ty,
        })
    }

    fn call_expr(&self, fn_ident: &syn::Ident, is_async: bool) -> proc_macro2::TokenStream {
        let mut args: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut prelude: Vec<proc_macro2::TokenStream> = Vec::new();

        if self.has_lxapp {
            args.push(quote! { __lingxia_lxapp });
        }

        if let Some(input_ty) = &self.input_ty {
            prelude.push(quote! {
                let __lingxia_payload: #input_ty =
                    match ::lingxia::host::parse_input(__lingxia_input.as_deref()) {
                        Ok(v) => v,
                        Err(e) => {
                            __lingxia_ctx.close_with("INVALID_PARAMS", e.to_string());
                            return;
                        }
                    };
            });
            args.push(quote! { __lingxia_payload });
        }

        args.push(quote! { __lingxia_ctx });

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
    input_fn: ItemFn,
) -> proc_macro2::TokenStream {
    let fn_ident = input_fn.sig.ident.clone();
    let helper_ident = format_ident!("{}_host", fn_ident);
    let handler_ident = format_ident!("__LingxiaChannelHandler_{}", fn_ident);
    let namespace_lit = LitStr::new(namespace, route_lit.span());
    let method_lit = LitStr::new(method, route_lit.span());

    let plan = match ChannelFnPlan::from_fn(&input_fn) {
        Ok(p) => p,
        Err(err) => return err.to_compile_error(),
    };

    let call_expr = plan.call_expr(&fn_ident, input_fn.sig.asyncness.is_some());
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
                __lingxia_lxapp: std::sync::Arc<::lingxia::LxApp>,
                __lingxia_ctx: ::lingxia::host::ChannelContext,
                __lingxia_input: Option<String>,
            ) {
                ::lingxia::tokio::task::spawn(async move {
                    let __lingxia_ctx =
                        __lingxia_ctx.with_types::<#inbound_ty, #outbound_ty>();
                    #call_expr
                });
            }
        }

        #[doc(hidden)]
        pub fn #helper_ident() -> ::lingxia::host::HostRegistrationEntry {
            ::lingxia::host::HostRegistrationEntry::Channel(
                ::lingxia::host::ChannelRegistration::new(
                    #namespace_lit,
                    #method_lit,
                    std::sync::Arc::new(#handler_ident),
                )
            )
        }
    }
}
