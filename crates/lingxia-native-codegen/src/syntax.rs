//! Classifies the parameters of a `#[native]` function by type.
//!
//! The macro and the TypeScript codegen both read routes through this module, so a
//! parameter can never be a payload to one and an extractor to the other.

use syn::spanned::Spanned;
use syn::{
    FnArg, GenericArgument, PatType, PathArguments, ReturnType, Signature, Type, TypeParamBound,
};

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
        if !matches!(param, Param::PageState(_)) {
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

/// The event type of a `watch` route: `T` in `-> Result<impl Stream<Item = T>, E>`.
pub fn watch_event(sig: &Signature) -> syn::Result<Type> {
    let shape_error = |span| {
        syn::Error::new(
            span,
            "a `watch` route returns `Result<impl Stream<Item = Event>, E>`; its first item is the current state",
        )
    };
    let ReturnType::Type(_, ty) = &sig.output else {
        return Err(shape_error(sig.ident.span()));
    };
    let ok = match last_segment(ty) {
        Some(segment) if segment.ident == "Result" => generics(ty)?.into_iter().next(),
        _ => None,
    };
    let Some(Type::ImplTrait(stream)) = ok else {
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
        for wrong in [
            "async fn f() -> impl Stream<Item = Ev> {}",
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
}
