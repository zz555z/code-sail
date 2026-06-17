use anyhow::{Context, Result};

pub(crate) async fn run_background_task<T>(
    label: &str,
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .with_context(|| format!("{} panicked or was cancelled", label))?
}
