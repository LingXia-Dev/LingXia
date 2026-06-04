/// Internal helper macro for defining synchronous Host APIs.
///
/// Not exported; only usable within `lingxia-lxapp`.
macro_rules! host_api {
    // With parameter
    ($name:ident, $input:ty, $output:ty, |$lxapp:ident, $param:ident| $body:block) => {
        pub(crate) struct $name;

        impl $crate::host::HostHandler for $name {
            fn call<'a>(
                &'a self,
                $lxapp: std::sync::Arc<$crate::LxApp>,
                input: Option<String>,
                _cancel: $crate::host::HostCancel,
            ) -> $crate::host::HostFuture<'a> {
                Box::pin(async move {
                    let $param: $input = $crate::host::parse_input(input.as_deref())?;
                    // Wrap in a closure so `return` inside `$body` returns from the closure,
                    // not from this async block (which must return JSON string).
                    #[allow(clippy::redundant_closure_call)]
                    let result: Result<$output, $crate::LxAppError> = (|| $body)();
                    $crate::host::serialize_result(result)
                })
            }
        }
    };

    // No parameter
    ($name:ident, $output:ty, |$lxapp:ident| $body:block) => {
        pub(crate) struct $name;

        impl $crate::host::HostHandler for $name {
            fn call<'a>(
                &'a self,
                $lxapp: std::sync::Arc<$crate::LxApp>,
                _input: Option<String>,
                _cancel: $crate::host::HostCancel,
            ) -> $crate::host::HostFuture<'a> {
                Box::pin(async move {
                    let result: Result<$output, $crate::LxAppError> = (|| $body)();
                    $crate::host::serialize_result(result)
                })
            }
        }
    };
}

/// Internal helper macro for defining async Host APIs with cancel.
///
/// Not exported; only usable within `lingxia-lxapp`.
macro_rules! host_api_async {
    // With parameter
    ($name:ident, $input:ty, $output:ty, |$lxapp:ident, $param:ident, $cancel:ident| async $body:block) => {
        pub(crate) struct $name;

        impl $crate::host::HostHandler for $name {
            fn call<'a>(
                &'a self,
                $lxapp: std::sync::Arc<$crate::LxApp>,
                input: Option<String>,
                mut $cancel: $crate::host::HostCancel,
            ) -> $crate::host::HostFuture<'a> {
                Box::pin(async move {
                    let $param: $input = $crate::host::parse_input(input.as_deref())?;
                    // Wrap in an async block so `return` inside `$body` returns from the inner
                    // block, not from this async block (which must return JSON string).
                    let result: Result<$output, $crate::LxAppError> = (async move { $body }).await;
                    $crate::host::serialize_result(result)
                })
            }
        }
    };

    // No parameter
    ($name:ident, $output:ty, |$lxapp:ident, $cancel:ident| async $body:block) => {
        pub(crate) struct $name;

        impl $crate::host::HostHandler for $name {
            fn call<'a>(
                &'a self,
                $lxapp: std::sync::Arc<$crate::LxApp>,
                _input: Option<String>,
                mut $cancel: $crate::host::HostCancel,
            ) -> $crate::host::HostFuture<'a> {
                Box::pin(async move {
                    let result: Result<$output, $crate::LxAppError> = (async move { $body }).await;
                    $crate::host::serialize_result(result)
                })
            }
        }
    };
}

macro_rules! register_host_module {
    ($namespace:literal, { $($method:literal => $handler:expr),+ $(,)? }) => {{
        $(
            $crate::host::register_host_route($namespace, $method, $handler);
        )+
    }};
}
