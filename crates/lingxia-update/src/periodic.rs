//! `update.checkInterval`: the periodic re-check shared by the host app and
//! the home lxapp.

use std::future::Future;
use std::ops::ControlFlow;
use std::time::Duration;

/// Wait one interval (plus jitter), run `step`, and repeat until it breaks.
/// The next wait starts only after `step` finishes, so checks never overlap.
pub async fn run_periodic_checks<F, Fut>(interval: Duration, mut step: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ControlFlow<()>>,
{
    loop {
        tokio::time::sleep(lingxia_app_context::update::next_check_delay(interval)).await;
        if step().await.is_break() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_periodic_checks;
    use std::ops::ControlFlow;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::time::Instant;

    const HOUR: Duration = Duration::from_secs(3600);

    /// Runs the loop on paused time and returns when each step started and
    /// ended, relative to the loop's start.
    async fn run(steps: usize, step_takes: Duration) -> Vec<(Duration, Duration)> {
        let start = Instant::now();
        let log = Arc::new(Mutex::new(Vec::new()));
        let log_for_loop = log.clone();
        run_periodic_checks(HOUR, move || {
            let log = log_for_loop.clone();
            async move {
                let began = start.elapsed();
                tokio::time::sleep(step_takes).await;
                let mut log = log.lock().unwrap();
                log.push((began, start.elapsed()));
                if log.len() == steps {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }
        })
        .await;
        Arc::try_unwrap(log).unwrap().into_inner().unwrap()
    }

    fn within_one_interval(wait: Duration) -> bool {
        wait >= HOUR && wait < HOUR + HOUR / 10
    }

    #[tokio::test(start_paused = true)]
    async fn first_check_waits_a_full_interval() {
        let log = run(1, Duration::ZERO).await;
        assert!(within_one_interval(log[0].0), "{:?}", log[0].0);
    }

    #[tokio::test(start_paused = true)]
    async fn checks_repeat_until_the_step_breaks() {
        let log = run(3, Duration::ZERO).await;
        assert_eq!(log.len(), 3);
        for pair in log.windows(2) {
            let wait = pair[1].0 - pair[0].1;
            assert!(within_one_interval(wait), "{wait:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn next_wait_starts_after_a_slow_check() {
        let slow = Duration::from_secs(20 * 60);
        let log = run(2, slow).await;
        assert_eq!(log[0].1 - log[0].0, slow);
        let wait = log[1].0 - log[0].1;
        assert!(within_one_interval(wait), "{wait:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_break_stops_the_loop() {
        let calls = Arc::new(Mutex::new(0));
        let counted = calls.clone();
        run_periodic_checks(HOUR, move || {
            let counted = counted.clone();
            async move {
                *counted.lock().unwrap() += 1;
                ControlFlow::Break(())
            }
        })
        .await;
        tokio::time::sleep(HOUR * 10).await;
        assert_eq!(*calls.lock().unwrap(), 1);
    }
}
