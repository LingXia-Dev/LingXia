use rong::{JSContext, JSRuntimeService};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct ContextFuture<F> {
    // Field order is intentional: drop the JS future before its context owner.
    future: Pin<Box<F>>,
    _context: JSContext,
}

impl<F: Future> Future for ContextFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().future.as_mut().poll(cx)
    }
}

pub(super) fn spawn<F, Fut>(ctx: &JSContext, task: F)
where
    F: FnOnce(JSContext) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let task_context = ctx.clone();
    ctx.spawn_task(ContextFuture {
        future: Box::pin(task(task_context)),
        // Rong JS values do not retain their JSContext wrapper. This field is
        // dropped after the future on completion and cancellation.
        _context: ctx.clone(),
    });
}

pub(super) async fn shutdown(ctx: &JSContext) {
    // Rong's callback timers are runtime-scoped even though their JSFunc values
    // belong to one context. Drain them before that context is released.
    ctx.runtime()
        .get_or_init_service::<rong_timer::TimerRegistry>()
        .on_shutdown();

    ctx.shutdown_tasks().await;
}
