use super::*;
use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};

struct PendingInsert {
    dropped: Arc<AtomicBool>,
}

impl Future for PendingInsert {
    type Output = Result<(), VerbalixError>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for PendingInsert {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn detached_history_returns_before_pending_insert_and_timeout_drops_it() {
    let dropped = Arc::new(AtomicBool::new(false));
    tokio::time::timeout(Duration::from_millis(20), async {
        spawn_history_insert(
            PendingInsert {
                dropped: dropped.clone(),
            },
            Duration::from_millis(5),
        );
    })
    .await
    .expect("detached persistence must not delay the primary result");

    tokio::time::timeout(Duration::from_millis(100), async {
        while !dropped.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("timeout must cancel and drop the pending insert");
}

#[tokio::test]
async fn history_outcomes_preserve_success_failure_and_timeout_categories() {
    assert!(matches!(
        history_insert_outcome(async { Ok(()) }, Duration::from_secs(1)).await,
        HistoryInsertOutcome::Inserted
    ));
    assert!(matches!(
        history_insert_outcome(
            async { Err(VerbalixError::ProviderRejected) },
            Duration::from_secs(1)
        )
        .await,
        HistoryInsertOutcome::Failed(VerbalixError::ProviderRejected)
    ));
    assert!(matches!(
        history_insert_outcome(
            std::future::pending::<Result<(), VerbalixError>>(),
            Duration::from_millis(1)
        )
        .await,
        HistoryInsertOutcome::TimedOut
    ));
}
