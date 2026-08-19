use std::sync::LazyLock;
use std::io;

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;
use tokio_util::sync::CancellationToken;

//------------------------------------------------------------------------------
// STRUCT: Task
//------------------------------------------------------------------------------
pub struct Task<T> {
    pub join_handle: JoinHandle<T>,
    pub cancel_token: CancellationToken
}

//------------------------------------------------------------------------------
// STRUCT: TokioManager
//------------------------------------------------------------------------------
pub struct TokioManager;

impl TokioManager {
    //---------------------------------------
    // Runtime function
    //---------------------------------------
    fn runtime() -> &'static Runtime {
        static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
            Runtime::new().expect("Failed to set up tokio runtime")
        });

        &RUNTIME
    }

    //---------------------------------------
    // Tracker function
    //---------------------------------------
    fn tracker() -> &'static TaskTracker {
        static TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);

        &TRACKER
    }

    //---------------------------------------
    // Token function
    //---------------------------------------
    fn token() -> &'static CancellationToken {
        static TOKEN: LazyLock<CancellationToken> = LazyLock::new(CancellationToken::new);

        &TOKEN
    }

    //---------------------------------------
    // Spawn function
    //---------------------------------------
    pub fn spawn<F, Fut, T>(func: F) -> Task<io::Result<T>>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let child_token = Self::token().child_token();
        let token = child_token.clone();

        let join_handle = Self::tracker().spawn_on(
            async move {
                tokio::select! {
                    () = token.cancelled() => Err(io::Error::other("Cancelled by user")),
                    result = func(token.clone()) => Ok(result)
                }
            },
            Self::runtime().handle()
        );

        Task {
            join_handle,
            cancel_token: child_token
        }
    }

    //---------------------------------------
    // Spawn blocking function
    //---------------------------------------
    pub fn spawn_blocking<F, T>(func: F) -> Task<io::Result<T>>
    where
        F: FnOnce(CancellationToken) -> T + Send + 'static,
        T: Send + 'static,
    {
        let child_token = Self::token().child_token();
        let token = child_token.clone();

        let join_handle = Self::tracker().spawn_on(
            async move {
                let token_clone = token.clone();
                let blocking_handle = tokio::task::spawn_blocking(move || func(token_clone));

                tokio::select! {
                    () = token.cancelled() => Err(io::Error::other("Cancelled by user")),
                    result = blocking_handle => Ok(result.expect("Failed to complete tokio task"))
                }
            },
            Self::runtime().handle(),
        );

        Task {
            join_handle,
            cancel_token: child_token
        }
    }

    //---------------------------------------
    // Spawn blocking function
    //---------------------------------------
    pub fn shutdown() {
        let tracker = Self::tracker();

        Self::token().cancel();
        tracker.close();

        Self::runtime().block_on(async move {
            tracker.wait().await;
        });
    }
}
