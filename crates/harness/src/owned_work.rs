//! Local work belongs to the host after admission, not its HTTP observer.
//! Dropping the waiter detaches the task; process shutdown still interrupts it.
use anyhow::{anyhow, Result};
use std::future::Future;

pub async fn wait<F, T>(work: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(work).await.map_err(|_| {
        anyhow!("Owned local operation exited unexpectedly; inspect its retained checkpoint")
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::{
        io::AsyncWriteExt,
        sync::Notify,
        time::{timeout, Duration},
    };

    struct ObserverDropped(Arc<Notify>);
    impl Drop for ObserverDropped {
        fn drop(&mut self) {
            self.0.notify_one();
        }
    }
    #[tokio::test]
    async fn actual_http_disconnect_drops_observer_but_keeps_one_durable_operation() {
        let root = std::env::temp_dir().join(format!("owned-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let database = root.join("work.sqlite3");
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE work(status TEXT); INSERT INTO work VALUES('pending')")
            .unwrap();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let committed = Arc::new(Notify::new());
        let dropped = Arc::new(Notify::new());
        let route = {
            let (started, release, committed, dropped, database) = (
                started.clone(),
                release.clone(),
                committed.clone(),
                dropped.clone(),
                database.clone(),
            );
            axum::routing::post(move || {
                let (started, release, committed, dropped, database) = (
                    started.clone(),
                    release.clone(),
                    committed.clone(),
                    dropped.clone(),
                    database.clone(),
                );
                async move {
                    let _observer = ObserverDropped(dropped);
                    wait(async move {
                        started.notify_one();
                        release.notified().await;
                        let count = rusqlite::Connection::open(database)?.execute(
                            "UPDATE work SET status='completed' WHERE status='pending'",
                            [],
                        )?;
                        committed.notify_one();
                        Ok(count)
                    })
                    .await
                    .unwrap()
                    .to_string()
                }
            })
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, axum::Router::new().route("/turn", route))
                .await
                .unwrap();
        });
        let mut observer = tokio::net::TcpStream::connect(address).await.unwrap();
        observer
            .write_all(b"POST /turn HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        timeout(Duration::from_secs(5), started.notified())
            .await
            .unwrap();
        drop(observer);
        timeout(Duration::from_secs(5), dropped.notified())
            .await
            .expect("the HTTP observer must actually be dropped before the work is released");
        release.notify_one();
        timeout(Duration::from_secs(5), committed.notified())
            .await
            .expect("host-owned work must survive its disconnected HTTP waiter");
        let status: String = rusqlite::Connection::open(&database)
            .unwrap()
            .query_row("SELECT status FROM work", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "completed");
        server.abort();
        let _ = server.await;
        std::fs::remove_dir_all(root).unwrap();
    }
}
