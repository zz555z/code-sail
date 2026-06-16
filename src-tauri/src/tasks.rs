use anyhow::{Context, Result};
use std::thread;

pub(crate) async fn run_background_task<T>(
    name: &str,
    task: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T>
where
    T: Send + 'static,
{
    let (sender, receiver) = tokio::sync::oneshot::channel();

    thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let _ = sender.send(task());
        })
        .with_context(|| format!("failed to spawn {name} thread"))?;

    receiver
        .await
        .with_context(|| format!("{name} thread exited without result"))?
}
