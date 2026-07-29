use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use tokio::sync::Notify;

#[derive(Debug)]
struct CooperativeCancellationTokenInner {
    cancelled: AtomicBool,
    notify: Notify,
}

#[derive(Clone, Debug)]
pub(crate) struct CooperativeCancellationToken {
    inner: Arc<CooperativeCancellationTokenInner>,
}

impl CooperativeCancellationToken {
    fn new() -> Self {
        Self {
            inner: Arc::new(CooperativeCancellationTokenInner {
                cancelled: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }

    pub(crate) fn cancel(&self) -> bool {
        if self.inner.cancelled.swap(true, Ordering::AcqRel) {
            return false;
        }

        self.inner.notify.notify_waiters();
        true
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        loop {
            if self.is_cancelled() {
                return;
            }

            let notified = self.inner.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CooperativeCancellationScope {
    BackgroundTask,
    BatchGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CooperativeCancellationKey {
    scope: CooperativeCancellationScope,
    task_id: String,
}

impl CooperativeCancellationKey {
    fn new(scope: CooperativeCancellationScope, task_id: impl Into<String>) -> Self {
        Self {
            scope,
            task_id: task_id.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveRegistration {
    registration_id: u64,
    token: CooperativeCancellationToken,
}

#[derive(Clone, Debug)]
pub(crate) struct CooperativeCancellationRegistry {
    registrations: Arc<RwLock<HashMap<CooperativeCancellationKey, ActiveRegistration>>>,
    next_registration_id: Arc<AtomicU64>,
}

impl Default for CooperativeCancellationRegistry {
    fn default() -> Self {
        Self {
            registrations: Arc::new(RwLock::new(HashMap::new())),
            next_registration_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl CooperativeCancellationRegistry {
    pub(crate) fn register(
        &self,
        scope: CooperativeCancellationScope,
        task_id: impl Into<String>,
    ) -> CooperativeCancellationRegistration {
        let key = CooperativeCancellationKey::new(scope, task_id);
        let registration_id = self.next_registration_id.fetch_add(1, Ordering::Relaxed);
        let token = CooperativeCancellationToken::new();
        let previous_registration = {
            let mut registrations = self.write_registrations();
            registrations.insert(
                key.clone(),
                ActiveRegistration {
                    registration_id,
                    token: token.clone(),
                },
            )
        };
        if let Some(previous_registration) = previous_registration {
            previous_registration.token.cancel();
        }

        CooperativeCancellationRegistration {
            registry: self.clone(),
            key,
            registration_id,
            token,
        }
    }

    pub(crate) fn cancel(&self, scope: CooperativeCancellationScope, task_id: &str) -> bool {
        let key = CooperativeCancellationKey::new(scope, task_id);
        let token = self
            .read_registrations()
            .get(&key)
            .map(|registration| registration.token.clone());

        token.map(|token| token.cancel()).unwrap_or(false)
    }

    fn remove_if_current(&self, key: &CooperativeCancellationKey, registration_id: u64) -> bool {
        let mut registrations = self.write_registrations();
        let is_current = registrations
            .get(key)
            .map(|registration| registration.registration_id == registration_id)
            .unwrap_or(false);
        if is_current {
            registrations.remove(key);
        }
        is_current
    }

    fn read_registrations(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<CooperativeCancellationKey, ActiveRegistration>>
    {
        self.registrations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_registrations(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, HashMap<CooperativeCancellationKey, ActiveRegistration>>
    {
        self.registrations
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    fn contains(&self, scope: CooperativeCancellationScope, task_id: &str) -> bool {
        self.read_registrations()
            .contains_key(&CooperativeCancellationKey::new(scope, task_id))
    }
}

pub(crate) struct CooperativeCancellationRegistration {
    registry: CooperativeCancellationRegistry,
    key: CooperativeCancellationKey,
    registration_id: u64,
    token: CooperativeCancellationToken,
}

impl CooperativeCancellationRegistration {
    pub(crate) fn token(&self) -> CooperativeCancellationToken {
        self.token.clone()
    }

    pub(crate) fn cleanup(&self) -> bool {
        self.registry
            .remove_if_current(&self.key, self.registration_id)
    }
}

impl Drop for CooperativeCancellationRegistration {
    fn drop(&mut self) {
        self.registry
            .remove_if_current(&self.key, self.registration_id);
    }
}

pub(crate) fn global_cooperative_cancellation_registry() -> &'static CooperativeCancellationRegistry
{
    static REGISTRY: OnceLock<CooperativeCancellationRegistry> = OnceLock::new();
    REGISTRY.get_or_init(CooperativeCancellationRegistry::default)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CooperativeCancellationRegistry, CooperativeCancellationScope};

    #[tokio::test]
    async fn token_cancel_is_monotonic_and_wakes_waiters() {
        let registry = CooperativeCancellationRegistry::default();
        let registration =
            registry.register(CooperativeCancellationScope::BackgroundTask, "task-waiter");
        let token = registration.token();
        let waiter_token = token.clone();
        let waiter = tokio::spawn(async move {
            waiter_token.cancelled().await;
        });

        tokio::task::yield_now().await;
        assert!(registry.cancel(CooperativeCancellationScope::BackgroundTask, "task-waiter"));
        assert!(!registry.cancel(CooperativeCancellationScope::BackgroundTask, "task-waiter"));
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("cancel should wake waiter")
            .expect("waiter should complete");
        assert!(token.is_cancelled());
        tokio::time::timeout(Duration::from_millis(50), token.cancelled())
            .await
            .expect("cancelled token should resolve immediately");
    }

    #[test]
    fn old_cleanup_does_not_remove_replacement_registration() {
        let registry = CooperativeCancellationRegistry::default();
        let old = registry.register(
            CooperativeCancellationScope::BatchGeneration,
            "batch-replaced",
        );
        let old_token = old.token();
        let replacement = registry.register(
            CooperativeCancellationScope::BatchGeneration,
            "batch-replaced",
        );
        let replacement_token = replacement.token();

        assert!(old_token.is_cancelled());
        assert!(!replacement_token.is_cancelled());
        assert!(!old.cleanup());
        assert!(registry.contains(
            CooperativeCancellationScope::BatchGeneration,
            "batch-replaced"
        ));
        assert!(registry.cancel(
            CooperativeCancellationScope::BatchGeneration,
            "batch-replaced"
        ));
        assert!(replacement_token.is_cancelled());
        assert!(replacement.cleanup());
        assert!(!replacement.cleanup());
        assert!(!registry.contains(
            CooperativeCancellationScope::BatchGeneration,
            "batch-replaced"
        ));
    }

    #[test]
    fn dropping_registration_cleans_up_only_current_instance() {
        let registry = CooperativeCancellationRegistry::default();
        {
            let _registration = registry.register(
                CooperativeCancellationScope::BackgroundTask,
                "task-drop-cleanup",
            );
            assert!(registry.contains(
                CooperativeCancellationScope::BackgroundTask,
                "task-drop-cleanup"
            ));
        }

        assert!(!registry.contains(
            CooperativeCancellationScope::BackgroundTask,
            "task-drop-cleanup"
        ));
        assert!(!registry.cancel(
            CooperativeCancellationScope::BackgroundTask,
            "task-drop-cleanup"
        ));
    }
}
