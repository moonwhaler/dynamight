//! Batched writes of run log lines to the logs database.
//!
//! Inserting every line on its own made verbose runs slow: rsync prints a line
//! per file, and the reader waited for each INSERT before reading on, so rsync
//! blocked on a full pipe. Lines now go through a channel to a single writer
//! task, which stores them in batches inside one transaction. One writer also
//! keeps lines in order (History sorts by insert ID).

use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, SqlitePool};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

use crate::models::LogLevel;

/// Lines held at most before senders wait (backpressure instead of growing
/// memory while the database is slow).
const QUEUE_CAPACITY: usize = 8192;
/// Most lines written in one INSERT (5 bound values each, well below
/// SQLite's limit on bound variables).
const MAX_BATCH: usize = 500;
/// How long the writer collects more lines before writing a batch.
const BATCH_WINDOW: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct LogRow {
    pub run_id: i64,
    pub level: LogLevel,
    pub message: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

enum Command {
    Write(LogRow),
    /// Answered once every line sent before it is stored
    Flush(oneshot::Sender<()>),
}

/// Handle to the writer task; cheap to clone.
#[derive(Clone)]
pub struct LogWriter {
    tx: mpsc::Sender<Command>,
}

impl LogWriter {
    /// Start the writer task. It ends when the last handle is dropped, after
    /// storing what it has received.
    pub fn spawn(logs_db: SqlitePool) -> Self {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        tokio::spawn(run_writer(logs_db, rx));
        Self { tx }
    }

    /// Queue a line, waiting while the queue is full.
    pub async fn write(&self, row: LogRow) {
        let _ = self.tx.send(Command::Write(row)).await;
    }

    /// Queue a line from synchronous code.
    ///
    /// Only for low-volume callers: when the queue is full the line is sent
    /// from a separate task and may end up after lines queued later.
    pub fn write_now(&self, row: LogRow) {
        if let Err(mpsc::error::TrySendError::Full(cmd)) = self.tx.try_send(Command::Write(row)) {
            let tx = self.tx.clone();
            tokio::spawn(async move {
                let _ = tx.send(cmd).await;
            });
        }
    }

    /// Wait until every line queued so far is stored (or the write failed).
    pub async fn flush(&self) {
        let (done_tx, done_rx) = oneshot::channel();
        if self.tx.send(Command::Flush(done_tx)).await.is_ok() {
            let _ = done_rx.await;
        }
    }
}

async fn run_writer(logs_db: SqlitePool, mut rx: mpsc::Receiver<Command>) {
    let mut batch: Vec<LogRow> = Vec::with_capacity(MAX_BATCH);
    let mut flushes: Vec<oneshot::Sender<()>> = Vec::new();

    while let Some(first) = rx.recv().await {
        let mut closed = false;
        take(first, &mut batch, &mut flushes);

        // Collect what arrives within the window, unless a flush waits
        let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
        while flushes.is_empty() && batch.len() < MAX_BATCH {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(cmd)) => take(cmd, &mut batch, &mut flushes),
                Ok(None) => {
                    closed = true;
                    break;
                }
                Err(_) => break,
            }
        }

        if !batch.is_empty() {
            if let Err(e) = insert_batch(&logs_db, &batch).await {
                tracing::warn!("Failed to store {} log line(s): {}", batch.len(), e);
            }
            batch.clear();
        }
        for done in flushes.drain(..) {
            let _ = done.send(());
        }
        if closed {
            break;
        }
    }
}

fn take(cmd: Command, batch: &mut Vec<LogRow>, flushes: &mut Vec<oneshot::Sender<()>>) {
    match cmd {
        Command::Write(row) => batch.push(row),
        Command::Flush(done) => flushes.push(done),
    }
}

async fn insert_batch(logs_db: &SqlitePool, rows: &[LogRow]) -> sqlx::Result<()> {
    let mut tx = logs_db.begin().await?;
    for chunk in rows.chunks(MAX_BATCH) {
        let mut qb = QueryBuilder::new("INSERT INTO log_entries (job_run_id, level, message, source, timestamp) ");
        qb.push_values(chunk, |mut b, row| {
            b.push_bind(row.run_id)
                .push_bind(row.level.as_str())
                .push_bind(&row.message)
                .push_bind(&row.source)
                .push_bind(row.timestamp);
        });
        qb.build().execute(&mut *tx).await?;
    }
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn logs_db() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_logs_migrations(&pool).await.unwrap();
        pool
    }

    fn row(run_id: i64, n: usize) -> LogRow {
        LogRow {
            run_id,
            level: LogLevel::Info,
            message: format!("line {n}"),
            source: "test".into(),
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn stores_all_lines_in_order_after_flush() {
        let db = logs_db().await;
        let writer = LogWriter::spawn(db.clone());
        for n in 0..1234 {
            writer.write(row(7, n)).await;
        }
        writer.write_now(row(7, 1234));
        writer.flush().await;

        let messages: Vec<(String,)> =
            sqlx::query_as("SELECT message FROM log_entries WHERE job_run_id = 7 ORDER BY id")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(messages.len(), 1235);
        assert!(messages.iter().enumerate().all(|(n, (m,))| *m == format!("line {n}")));
    }

    #[tokio::test]
    async fn flush_without_lines_returns() {
        let writer = LogWriter::spawn(logs_db().await);
        tokio::time::timeout(Duration::from_secs(2), writer.flush()).await.unwrap();
    }

    #[tokio::test]
    async fn dropping_the_last_handle_stores_queued_lines() {
        let db = logs_db().await;
        let writer = LogWriter::spawn(db.clone());
        writer.write(row(1, 0)).await;
        drop(writer);
        for _ in 0..50 {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM log_entries").fetch_one(&db).await.unwrap();
            if n == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("queued line was not stored");
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

    /// cargo test --lib log_writer::bench -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn per_line_inserts_vs_batched_writer() {
        const LINES: usize = 50_000;
        let dir = tempfile::tempdir().unwrap();
        let open = |name: &str| {
            SqliteConnectOptions::new()
                .filename(dir.path().join(name))
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
        };
        let old = SqlitePoolOptions::new().max_connections(10).connect_with(open("old.db")).await.unwrap();
        let new = SqlitePoolOptions::new().max_connections(10).connect_with(open("new.db")).await.unwrap();
        crate::db::run_logs_migrations(&old).await.unwrap();
        crate::db::run_logs_migrations(&new).await.unwrap();
        let row = |n: usize| LogRow {
            run_id: 1,
            level: LogLevel::Info,
            message: format!("photos/2026/IMG_{n:06}.jpg"),
            source: "rsync".into(),
            timestamp: Utc::now(),
        };

        let start = std::time::Instant::now();
        for n in 0..LINES {
            let r = row(n);
            sqlx::query("INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, ?, ?)")
                .bind(r.run_id)
                .bind(r.level.as_str())
                .bind(&r.message)
                .bind(&r.source)
                .bind(r.timestamp)
                .execute(&old)
                .await
                .unwrap();
        }
        let per_line = start.elapsed();

        let start = std::time::Instant::now();
        let writer = LogWriter::spawn(new.clone());
        for n in 0..LINES {
            writer.write(row(n)).await;
        }
        writer.flush().await;
        let batched = start.elapsed();

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM log_entries").fetch_one(&new).await.unwrap();
        assert_eq!(count as usize, LINES);
        println!("{LINES} lines: per-line {per_line:?}, batched {batched:?}");
    }
}
