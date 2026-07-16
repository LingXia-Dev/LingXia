use rong::{JSContext, JSContextService, JSRuntimeService};
use std::cell::RefCell;
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

#[derive(Clone, Default)]
struct ContextTaskRegistry {
    tasks: std::rc::Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>,
}

impl ContextTaskRegistry {
    fn track(&self, task: tokio::task::JoinHandle<()>) {
        let mut tasks = self.tasks.borrow_mut();
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);
    }

    fn take_all(&self) -> Vec<tokio::task::JoinHandle<()>> {
        self.tasks.borrow_mut().drain(..).collect()
    }

    fn abort_all(&self) {
        for task in self.take_all() {
            task.abort();
        }
    }

    async fn cancel_all(&self) {
        let tasks = self.take_all();
        for task in &tasks {
            task.abort();
        }
        for task in tasks {
            let _ = task.await;
        }
    }
}

impl JSContextService for ContextTaskRegistry {
    fn on_shutdown(&self) {
        self.abort_all();
    }
}

fn task_registry(ctx: &JSContext) -> ContextTaskRegistry {
    if ctx.get_service::<ContextTaskRegistry>().is_none() {
        ctx.set_service(ContextTaskRegistry::default());
    }
    ctx.get_service::<ContextTaskRegistry>()
        .expect("context task registry was inserted above")
        .clone()
}

pub(super) fn spawn<F, Fut>(ctx: &JSContext, task: F)
where
    F: FnOnce(JSContext) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let registry = task_registry(ctx);
    let task_context = ctx.clone();
    let handle = rong::spawn_local(ContextFuture {
        future: Box::pin(task(task_context)),
        // Rong JS values do not retain their JSContext wrapper. This field is
        // dropped after the future on completion and cancellation.
        _context: ctx.clone(),
    });
    registry.track(handle);
}

pub(super) async fn shutdown(ctx: &JSContext) {
    // Rong's callback timers are runtime-scoped even though their JSFunc values
    // belong to one context. Drain them before that context is released.
    ctx.runtime()
        .get_or_init_service::<rong_timer::TimerRegistry>()
        .on_shutdown();

    if let Some(registry) = ctx.get_service::<ContextTaskRegistry>() {
        registry.cancel_all().await;
    }
}
