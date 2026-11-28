//! Classifies the parameters of a `#[native]` function by type.
//!
//! The macro and the TypeScript codegen both read routes through this module, so a
//! parameter can never be a payload to one and an extractor to the other.

use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Expr, FnArg, GenericArgument, Lit, LitStr, PatType, PathArguments, ReturnType, Signature,
    Token, Type, TypeParamBound,
};

/// Who may call a route; immutable registration metadata, so a published contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAudience {
    AppSessionOnly,
    ControlAppOnly,
    ControlSurfaceOnly,
    ControlAppOrSurface,
}

impl RouteAudience {
    pub fn parse(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "app-session-only" => Ok(Self::AppSessionOnly),
            "control-app-only" => Ok(Self::ControlAppOnly),
            "control-surface-only" => Ok(Self::ControlSurfaceOnly),
            "control-app-or-surface" => Ok(Self::ControlAppOrSurface),
            other => Err(syn::Error::new_spanned(
                value,
                format!(
                    "unknown audience `{other}`; expected one of `app-session-only`, `control-app-only`, `control-surface-only`, or `control-app-or-surface`"
                ),
            )),
        }
    }

    /// The `lingxia::host::RouteAudience` variant.
    pub fn variant(self) -> &'static str {
        match self {
            Self::AppSessionOnly => "AppSessionOnly",
            Self::ControlAppOnly => "ControlAppOnly",
            Self::ControlSurfaceOnly => "ControlSurfaceOnly",
            Self::ControlAppOrSurface => "ControlAppOrSurface",
        }
    }
}

/// A route attribute's arguments, before `route_defaults!` fills the name and audience.
#[derive(Debug, Clone)]
pub struct RouteAttr {
    /// The route string: absolute with a dot, else a method under the file's prefix.
    pub name: Option<LitStr>,
    pub shape: Shape,
    pub audience: Option<RouteAudience>,
}

/// Parses `#[native("name"?, mode flags, audience = "…"?)]`.
pub fn parse_route_attr(
    args: &Punctuated<Expr, Token![,]>,
    macro_name: &str,
) -> syn::Result<RouteAttr> {
    let mut name = None;
    let mut mode: Option<(Shape, &Expr)> = None;
    let mut sync: Option<&Expr> = None;
    let mut audience = None;
    for (index, arg) in args.iter().enumerate() {
        match arg {
            Expr::Lit(lit) if index == 0 => {
                let Lit::Str(value) = &lit.lit else {
                    return Err(syn::Error::new_spanned(
                        &lit.lit,
                        format!("the route name in #[{macro_name}(...)] is a string"),
                    ));
                };
                check_route_name(value)?;
                name = Some(value.clone());
            }
            Expr::Path(path) if path.path.is_ident("blocking") => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "a plain `fn` route runs on a blocking thread; remove `blocking`",
                ));
            }
            Expr::Path(path) if path.path.is_ident("sync") => {
                if sync.replace(arg).is_some() {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate sync flag in #[{macro_name}(...)]"),
                    ));
                }
            }
            Expr::Path(path)
                if ["stream", "watch", "channel"]
                    .iter()
                    .any(|flag| path.path.is_ident(flag)) =>
            {
                let shape = if path.path.is_ident("stream") {
                    Shape::Stream
                } else if path.path.is_ident("watch") {
                    Shape::Watch
                } else {
                    Shape::Channel
                };
                if mode.replace((shape, arg)).is_some() {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate or conflicting mode flag in #[{macro_name}(...)]"),
                    ));
                }
            }
            Expr::Assign(assign) if matches!(assign.left.as_ref(), Expr::Path(path) if path.path.is_ident("audience")) =>
            {
                if audience.is_some() {
                    return Err(syn::Error::new_spanned(
                        arg,
                        format!("duplicate audience option in #[{macro_name}(...)]"),
                    ));
                }
                audience = Some(RouteAudience::parse(string_value(
                    &assign.right,
                    "audience",
                    macro_name,
                )?)?);
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    arg,
                    format!(
                        "expected a route name, a mode flag (`sync`, `stream`, `watch`, or `channel`) or `audience = \"…\"` in #[{macro_name}(...)]"
                    ),
                ));
            }
        }
    }
    let shape = match (mode, sync) {
        (Some(_), Some(flag)) => {
            return Err(syn::Error::new_spanned(
                flag,
                format!("sync is only supported for unary #[{macro_name}] handlers"),
            ));
        }
        (Some((shape, _)), None) => shape,
        (None, Some(_)) => Shape::Sync,
        (None, None) => Shape::Call,
    };
    Ok(RouteAttr {
        name,
        shape,
        audience,
    })
}

/// `route_defaults!(prefix = "…", audience = "…")`; either may be left out.
#[derive(Debug, Clone, Default)]
pub struct RouteDefaults {
    pub prefix: Option<String>,
    pub audience: Option<RouteAudience>,
}

pub fn parse_route_defaults(args: &Punctuated<Expr, Token![,]>) -> syn::Result<RouteDefaults> {
    const MACRO: &str = "route_defaults!";
    let mut defaults = RouteDefaults::default();
    for arg in args {
        let Expr::Assign(assign) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "expected `prefix = \"…\"` or `audience = \"…\"`",
            ));
        };
        let key = match assign.left.as_ref() {
            Expr::Path(path) if path.path.is_ident("prefix") => "prefix",
            Expr::Path(path) if path.path.is_ident("audience") => "audience",
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected `prefix` or `audience`",
                ));
            }
        };
        let value = string_value(&assign.right, key, MACRO)?;
        let repeated = if key == "prefix" {
            check_namespace(value, &value.value())?;
            defaults.prefix.replace(value.value()).is_some()
        } else {
            defaults
                .audience
                .replace(RouteAudience::parse(value)?)
                .is_some()
        };
        if repeated {
            return Err(syn::Error::new_spanned(
                arg,
                format!("duplicate `{key}` in {MACRO}"),
            ));
        }
    }
    if defaults.prefix.is_none() && defaults.audience.is_none() {
        return Err(syn::Error::new_spanned(
            args,
            "route_defaults! needs `prefix = \"…\"`, `audience = \"…\"`, or both",
        ));
    }
    Ok(defaults)
}

/// The full route name: an explicit name with a dot is absolute; otherwise the method,
/// given or from the fn name in camelCase, goes under `prefix`.
pub fn resolve_route_name(
    name: Option<&str>,
    fn_ident: &str,
    prefix: Option<&str>,
) -> Result<String, String> {
    match (name, prefix) {
        (Some(name), _) if name.contains('.') => Ok(name.to_string()),
        (Some(method), Some(prefix)) => Ok(format!("{prefix}.{method}")),
        (None, Some(prefix)) => Ok(format!("{prefix}.{}", camel_case(fn_ident))),
        (Some(method), None) => Err(format!(
            "route `{method}` has no namespace; write \"namespace.{method}\" or add `lingxia::route_defaults!(prefix = \"…\")` above it"
        )),
        (None, None) => Err(format!(
            "`{fn_ident}` has no route name; write #[native(\"namespace.method\")] or add `lingxia::route_defaults!(prefix = \"…\")` above it"
        )),
    }
}

fn camel_case(ident: &str) -> String {
    let ident = ident.strip_prefix("r#").unwrap_or(ident);
    let mut out = String::with_capacity(ident.len());
    let mut upper = false;
    for ch in ident.chars() {
        if ch == '_' {
            upper = !out.is_empty();
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn string_value<'a>(expr: &'a Expr, key: &str, macro_name: &str) -> syn::Result<&'a LitStr> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Ok(value),
        _ => Err(syn::Error::new_spanned(
            expr,
            format!("{key} must be a string literal in {macro_name}"),
        )),
    }
}

fn check_route_name(value: &LitStr) -> syn::Result<()> {
    let name = value.value();
    let (namespace, method) = match name.rsplit_once('.') {
        Some((namespace, method)) => (Some(namespace), method),
        None => (None, name.as_str()),
    };
    if let Some(namespace) = namespace {
        check_namespace(value, namespace)?;
    }
    if method.trim().is_empty() {
        return Err(syn::Error::new_spanned(
            value,
            "a route name needs a non-empty method",
        ));
    }
    Ok(())
}

fn check_namespace(span: &LitStr, namespace: &str) -> syn::Result<()> {
    if namespace.trim().is_empty() || namespace.split('.').any(|part| part.trim().is_empty()) {
        return Err(syn::Error::new_spanned(
            span,
            "a route namespace must not be empty",
        ));
    }
    if namespace == "channel" {
        return Err(syn::Error::new_spanned(
            span,
            "namespace 'channel' is reserved by the JS API; choose a different namespace",
        ));
    }
    Ok(())
}

/// How a route is invoked, from its attribute flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Call,
    Sync,
    Stream,
    Watch,
    Channel,
}

/// One parameter, in declaration order. Everything that is not an extractor is the payload.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // a handful per route, at build time
pub enum Param {
    /// `Arc<LxApp>`
    LxApp,
    /// `HostInvocationContext`
    Invocation,
    /// `HostCancel`
    Cancel,
    /// `PageState<T>`; the whole type.
    PageState(Type),
    /// `LxAppState<T>`; the whole type.
    LxAppState(Type),
    /// `Host<A>`; the addon type `A`.
    Host(Type),
    /// `Progress<Event>`; an absent generic stays `None`.
    Progress(Option<Type>),
    /// `ChannelContext<In, Out>`; one generic means both directions carry it.
    Channel {
        inbound: Option<Type>,
        outbound: Option<Type>,
    },
    Payload(Type),
}

/// The classified parameter list of one route function.
#[derive(Debug, Clone)]
pub struct Params(pub Vec<Param>);

impl Params {
    pub fn payload(&self) -> Option<&Type> {
        self.0.iter().find_map(|param| match param {
            Param::Payload(ty) => Some(ty),
            _ => None,
        })
    }

    /// The event type of a stream route's `Progress<Event>`.
    pub fn progress(&self) -> Option<Option<&Type>> {
        self.0.iter().find_map(|param| match param {
            Param::Progress(event) => Some(event.as_ref()),
            _ => None,
        })
    }

    pub fn channel(&self) -> Option<(Option<&Type>, Option<&Type>)> {
        self.0.iter().find_map(|param| match param {
            Param::Channel { inbound, outbound } => Some((inbound.as_ref(), outbound.as_ref())),
            _ => None,
        })
    }
}

/// Classifies `sig`'s parameters for a route of `shape`, in any order.
pub fn classify(sig: &Signature, shape: Shape) -> syn::Result<Params> {
    let mut params = Vec::with_capacity(sig.inputs.len());
    let mut payload: Option<&PatType> = None;
    let mut seen: Vec<&'static str> = Vec::new();

    for arg in &sig.inputs {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "#[native] does not support methods with a receiver",
            ));
        };
        let ty = &*arg.ty;
        let param = match extractor(ty)? {
            Some(param) => param,
            None => {
                if let Some(first) = payload {
                    let mut err = syn::Error::new_spanned(
                        arg,
                        "#[native] takes one payload argument, and this is a second; \
                         put both in one struct",
                    );
                    err.combine(syn::Error::new_spanned(first, "the first payload"));
                    return Err(err);
                }
                payload = Some(arg);
                params.push(Param::Payload(ty.clone()));
                continue;
            }
        };

        let name = name(&param);
        if !matches!(param, Param::PageState(_) | Param::LxAppState(_)) {
            if seen.contains(&name) {
                return Err(syn::Error::new_spanned(
                    arg,
                    format!("#[native] takes `{name}` only once"),
                ));
            }
            seen.push(name);
        }
        if let Some(message) = misplaced(&param, shape) {
            return Err(syn::Error::new_spanned(arg, message));
        }
        params.push(param);
    }

    let missing = match shape {
        Shape::Stream if !seen.contains(&"Progress") => Some(
            "#[native(..., stream)] function must take a `Progress<Event>` argument; its return value is the result",
        ),
        Shape::Channel if !seen.contains(&"ChannelContext") => {
            Some("#[native(..., channel)] function must take a `ChannelContext` argument")
        }
        _ => None,
    };
    if let Some(message) = missing {
        return Err(syn::Error::new(sig.ident.span(), message));
    }
    Ok(Params(params))
}

fn extractor(ty: &Type) -> syn::Result<Option<Param>> {
    let Some(last) = last_segment(ty) else {
        return Ok(None);
    };
    Ok(Some(match last.ident.to_string().as_str() {
        "Arc" if generics(ty)?.first().is_some_and(is_lxapp) => Param::LxApp,
        "HostInvocationContext" => Param::Invocation,
        "HostCancel" => Param::Cancel,
        "PageState" => Param::PageState(ty.clone()),
        "LxAppState" => Param::LxAppState(ty.clone()),
        "Host" => match generics(ty)?.as_slice() {
            [addon] => Param::Host(addon.clone()),
            _ => return Ok(None),
        },
        "Progress" => {
            let args = generics(ty)?;
            if args.len() > 1 {
                return Err(syn::Error::new_spanned(
                    ty,
                    "`Progress` takes one generic argument, the event; the route's return value is the result",
                ));
            }
            Param::Progress(args.into_iter().next())
        }
        "ChannelContext" => {
            let mut args = bounded(ty, "ChannelContext")?.into_iter();
            let inbound = args.next();
            let outbound = args.next().or_else(|| inbound.clone());
            Param::Channel { inbound, outbound }
        }
        _ => return Ok(None),
    }))
}

fn misplaced(param: &Param, shape: Shape) -> Option<&'static str> {
    match (param, shape) {
        (Param::Cancel, Shape::Sync) => {
            Some("#[native(..., sync)] cannot take HostCancel: a sync route is never cancelled")
        }
        (Param::Cancel, Shape::Stream | Shape::Watch | Shape::Channel) => Some(
            "HostCancel is for unary routes; a stream, watch or channel stops when its caller cancels",
        ),
        (Param::PageState(_), Shape::Stream | Shape::Watch | Shape::Channel) => {
            Some("PageState<T> is supported on unary routes only")
        }
        (Param::LxAppState(_), Shape::Sync) => Some(
            "#[native(..., sync)] cannot take LxAppState<T>: its first use awaits `open`; drop `sync`",
        ),
        (Param::Progress(_), shape) if shape != Shape::Stream => {
            Some("`Progress` needs `#[native(..., stream)]`")
        }
        (Param::Channel { .. }, shape) if shape != Shape::Channel => {
            Some("`ChannelContext` needs `#[native(..., channel)]`")
        }
        _ => None,
    }
}

fn name(param: &Param) -> &'static str {
    match param {
        Param::LxApp => "Arc<LxApp>",
        Param::Invocation => "HostInvocationContext",
        Param::Cancel => "HostCancel",
        Param::PageState(_) => "PageState",
        Param::LxAppState(_) => "LxAppState",
        Param::Host(_) => "Host",
        Param::Progress(_) => "Progress",
        Param::Channel { .. } => "ChannelContext",
        Param::Payload(_) => "payload",
    }
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(path) if path.qself.is_none() => path.path.segments.last(),
        _ => None,
    }
}

fn is_lxapp(ty: &Type) -> bool {
    last_segment(ty).is_some_and(|segment| segment.ident == "LxApp")
}

fn generics(ty: &Type) -> syn::Result<Vec<Type>> {
    let Some(PathArguments::AngleBracketed(args)) = last_segment(ty).map(|s| &s.arguments) else {
        return Ok(Vec::new());
    };
    args.args
        .iter()
        .map(|arg| match arg {
            GenericArgument::Type(ty) => Ok(ty.clone()),
            other => Err(syn::Error::new(
                other.span(),
                "only type generic arguments are supported here",
            )),
        })
        .collect()
}

fn bounded(ty: &Type, name: &str) -> syn::Result<Vec<Type>> {
    let args = generics(ty)?;
    if args.len() > 2 {
        return Err(syn::Error::new_spanned(
            ty,
            format!("`{name}` supports at most two generic arguments"),
        ));
    }
    Ok(args)
}

/// Whether a route returns a `Result` (any path ending in `Result`), or a plain value.
pub fn returns_result(sig: &Signature) -> bool {
    match &sig.output {
        ReturnType::Type(_, ty) => last_segment(ty).is_some_and(|s| s.ident == "Result"),
        ReturnType::Default => false,
    }
}

/// The event type of a `watch` route: `T` in `-> impl Stream<Item = T>`, bare or in a `Result`.
pub fn watch_event(sig: &Signature) -> syn::Result<Type> {
    let shape_error = |span| {
        syn::Error::new(
            span,
            "a `watch` route returns `impl Stream<Item = Event>`, or that in a `Result`; its first item is the current state",
        )
    };
    let ReturnType::Type(_, ty) = &sig.output else {
        return Err(shape_error(sig.ident.span()));
    };
    let stream = if returns_result(sig) {
        generics(ty)?.into_iter().next()
    } else {
        Some((**ty).clone())
    };
    let Some(Type::ImplTrait(stream)) = stream else {
        return Err(shape_error(ty.span()));
    };
    stream
        .bounds
        .iter()
        .find_map(|bound| {
            let TypeParamBound::Trait(bound) = bound else {
                return None;
            };
            let segment = bound.path.segments.last()?;
            if segment.ident != "Stream" {
                return None;
            }
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return None;
            };
            args.args.iter().find_map(|arg| match arg {
                GenericArgument::AssocType(item) if item.ident == "Item" => Some(item.ty.clone()),
                _ => None,
            })
        })
        .ok_or_else(|| shape_error(stream.span()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn parse(src: &str) -> Signature {
        syn::parse_str::<syn::ItemFn>(src).expect("fn").sig
    }

    fn kinds(params: &Params) -> Vec<&'static str> {
        params.0.iter().map(name).collect()
    }

    fn error(src: &str, shape: Shape) -> String {
        classify(&parse(src), shape)
            .expect_err("classification should fail")
            .to_string()
    }

    #[test]
    fn extractors_and_the_payload_are_found_in_any_order() {
        let params = classify(
            &parse(
                "fn f(cancel: HostCancel, input: Query, app: Arc<lingxia::LxApp>, \
                 state: PageState<S>, ctx: HostInvocationContext) {}",
            ),
            Shape::Call,
        )
        .expect("classify");
        assert_eq!(
            kinds(&params),
            [
                "HostCancel",
                "payload",
                "Arc<LxApp>",
                "PageState",
                "HostInvocationContext"
            ]
        );
        assert_eq!(
            params.payload().map(|ty| ty.to_token_stream().to_string()),
            Some("Query".into())
        );
    }

    #[test]
    fn a_context_may_lead_its_payload() {
        let stream = classify(
            &parse("async fn f(progress: Progress<Ev>, input: Query) {}"),
            Shape::Stream,
        )
        .expect("stream");
        let event = stream.progress().expect("progress");
        assert_eq!(event.unwrap().to_token_stream().to_string(), "Ev");
        assert!(stream.payload().is_some());

        let channel = classify(
            &parse("async fn f(ch: ChannelContext<Msg>, app: Arc<LxApp>) {}"),
            Shape::Channel,
        )
        .expect("channel");
        let (inbound, outbound) = channel.channel().expect("channel context");
        assert_eq!(inbound.unwrap().to_token_stream().to_string(), "Msg");
        assert_eq!(outbound.unwrap().to_token_stream().to_string(), "Msg");
        assert!(channel.payload().is_none());
    }

    #[test]
    fn a_second_payload_is_an_error_naming_both() {
        let err = classify(&parse("fn f(a: A, b: B) {}"), Shape::Call)
            .expect_err("two payloads")
            .into_iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();
        assert_eq!(err.len(), 2, "{err:?}");
        assert!(err[0].contains("a second"), "{err:?}");
        assert!(err[1].contains("the first payload"), "{err:?}");
    }

    #[test]
    fn misplaced_and_repeated_extractors_are_errors() {
        assert!(error("fn f(c: HostCancel) {}", Shape::Sync).contains("never cancelled"));
        assert!(
            error("fn f(c: HostCancel, p: Progress<E>) {}", Shape::Stream).contains("unary routes")
        );
        assert!(
            error(
                "fn f(p: PageState<S>, c: ChannelContext) {}",
                Shape::Channel
            )
            .contains("unary routes only")
        );
        assert!(error("fn f(p: Progress<E>) {}", Shape::Call).contains("stream)]"));
        assert!(error("fn f(p: Progress<E>) {}", Shape::Watch).contains("stream)]"));
        assert!(
            error("fn f(x: Query) {}", Shape::Stream).contains("must take a `Progress<Event>`")
        );
        assert!(
            error("fn f(x: Query) {}", Shape::Channel).contains("must take a `ChannelContext`")
        );
        assert!(error("fn f(a: Arc<LxApp>, b: Arc<LxApp>) {}", Shape::Call).contains("only once"));
        assert!(error("fn f(p: Progress<A, B>) {}", Shape::Stream).contains("one generic"));
        assert!(
            error("fn f(c: ChannelContext<A, B, C>) {}", Shape::Channel).contains("at most two")
        );
    }

    #[test]
    fn several_page_states_and_a_plain_arc_payload_are_allowed() {
        let params = classify(
            &parse("fn f(a: PageState<A>, b: PageState<B>, shared: Arc<Config>) {}"),
            Shape::Call,
        )
        .expect("classify");
        assert_eq!(kinds(&params), ["PageState", "PageState", "payload"]);
    }

    #[test]
    fn a_watch_event_is_the_streams_item() {
        let item = |src: &str| watch_event(&parse(src)).map(|ty| ty.to_token_stream().to_string());
        assert_eq!(
            item("async fn f() -> lingxia::Result<impl Stream<Item = Ev> + Send + 'static> {}")
                .unwrap(),
            "Ev"
        );
        assert_eq!(
            item("fn f(q: Query) -> Result<impl futures::Stream<Item = Vec<Ev>>, MyError> {}")
                .unwrap(),
            "Vec < Ev >"
        );
        assert_eq!(
            item("fn f() -> impl Stream<Item = Ev> + Send + 'static {}").unwrap(),
            "Ev"
        );
        for wrong in [
            "async fn f() -> Vec<Ev> {}",
            "async fn f() -> lingxia::Result<Vec<Ev>> {}",
            "async fn f() -> lingxia::Result<impl Iterator<Item = Ev>> {}",
            "async fn f() {}",
        ] {
            assert!(
                item(wrong)
                    .unwrap_err()
                    .to_string()
                    .contains("impl Stream<Item = Event>"),
                "{wrong}"
            );
        }
    }

    fn args(tokens: impl ToTokens) -> Punctuated<Expr, Token![,]> {
        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        syn::parse::Parser::parse2(parser, tokens.into_token_stream()).unwrap()
    }

    fn attr(tokens: impl ToTokens) -> syn::Result<RouteAttr> {
        parse_route_attr(&args(tokens), "native")
    }

    #[test]
    fn route_attributes_parse_every_audience_and_mode() {
        use quote::quote;
        let cases = [
            (quote!("demo.app"), None, Shape::Call),
            (quote!("demo.load", sync), None, Shape::Sync),
            (
                quote!("demo.host", audience = "control-app-only"),
                Some(RouteAudience::ControlAppOnly),
                Shape::Call,
            ),
            (
                quote!(stream, audience = "control-surface-only"),
                Some(RouteAudience::ControlSurfaceOnly),
                Shape::Stream,
            ),
            (
                quote!("any", audience = "control-app-or-surface", channel),
                Some(RouteAudience::ControlAppOrSurface),
                Shape::Channel,
            ),
            (
                quote!(watch, audience = "app-session-only"),
                Some(RouteAudience::AppSessionOnly),
                Shape::Watch,
            ),
        ];
        for (tokens, audience, shape) in cases {
            let parsed = attr(tokens).expect("route attribute");
            assert_eq!((parsed.audience, parsed.shape), (audience, shape));
        }
    }

    #[test]
    fn bad_route_attributes_are_errors() {
        use quote::quote;
        for (tokens, expected) in [
            (quote!("demo.x", audience = "guest"), "unknown audience"),
            (quote!("demo.x", audience = DEFAULT), "string literal"),
            (
                quote!(audience = "app-session-only", audience = "control-app-only"),
                "duplicate audience",
            ),
            (quote!(stream, watch), "conflicting mode"),
            (quote!(sync, stream), "sync is only supported"),
            (quote!("demo.x", blocking), "remove `blocking`"),
            (quote!("channel.x"), "reserved"),
            (quote!(".x"), "must not be empty"),
            (quote!("demo."), "non-empty method"),
            (quote!(stream, "demo.x"), "expected a route name"),
        ] {
            let error = attr(tokens).expect_err(expected).to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn route_names_resolve_against_the_prefix() {
        assert_eq!(
            resolve_route_name(None, "set_pinned", Some("bookmarks")).unwrap(),
            "bookmarks.setPinned"
        );
        assert_eq!(
            resolve_route_name(Some("export_html"), "export", Some("bookmarks")).unwrap(),
            "bookmarks.export_html"
        );
        assert_eq!(
            resolve_route_name(Some("other.get"), "get", Some("bookmarks")).unwrap(),
            "other.get"
        );
        assert!(resolve_route_name(None, "load", None).is_err());
        assert!(resolve_route_name(Some("load"), "load", None).is_err());
    }

    #[test]
    fn route_defaults_take_a_prefix_an_audience_or_both() {
        let parse = |tokens| parse_route_defaults(&args(tokens));
        use quote::quote;
        let both = parse(quote!(prefix = "a.b", audience = "control-app-only")).unwrap();
        assert_eq!(both.prefix.as_deref(), Some("a.b"));
        assert_eq!(both.audience, Some(RouteAudience::ControlAppOnly));
        for (tokens, expected) in [
            (quote!(), "needs `prefix"),
            (quote!(prefix = "a", prefix = "b"), "duplicate `prefix`"),
            (quote!(prefix = "channel"), "reserved"),
            (quote!(name = "a"), "expected `prefix` or `audience`"),
        ] {
            let error = parse(tokens).expect_err(expected).to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn a_route_returns_a_result_by_its_type_name() {
        let returns = |src: &str| returns_result(&parse(src));
        assert!(returns("fn f() -> lingxia::Result<u32> {}"));
        assert!(returns("fn f() -> anyhow::Result<()> {}"));
        assert!(returns("fn f() -> Result<Note, NotesError> {}"));
        assert!(!returns("fn f() -> String {}"));
        assert!(!returns("fn f() {}"));
    }
}
