//! The single boundary for executing system commands.
//!
//! The Go daemon had a `Runner` interface in every package; here there is one
//! shared trait. Monitors use [`OutputRunner`] (stdout only — parsers treat an
//! empty string as "unknown"), interactions use [`CombinedRunner`] (nmcli and
//! friends report failures on stderr, so callers need that text).
//!
//! Every call is bounded: [`Ctx`] carries a parent deadline (the daemon caps
//! requests at 30s) and each runner has its own default timeout. The shorter
//! of the two wins.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// A shared cancellation flag, the Rust equivalent of Go's cancellable
/// `context.Context`. Any holder can cancel; every derived `Ctx` sees it.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A cancellation/deadline token passed down the call tree.
///
/// Stubs in tests ignore it; real runners clamp their own timeout to whatever
/// remains here and kill the child when cancellation fires. This is what lets
/// daemon shutdown interrupt an in-flight Wi-Fi scan instead of waiting for
/// the scan's 25s timeout.
#[derive(Clone, Debug)]
pub struct Ctx {
    deadline: Option<Instant>,
    cancel: Option<CancelToken>,
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            deadline: None,
            cancel: None,
        }
    }

    /// Derive a child context with a timeout no later than this one's.
    pub fn with_timeout(&self, d: Duration) -> Self {
        let at = Instant::now() + d;
        let deadline = Some(match self.deadline {
            Some(parent) if parent < at => parent,
            _ => at,
        });
        Self {
            deadline,
            cancel: self.cancel.clone(),
        }
    }

    /// Attach a cancel token (preserving the deadline).
    pub fn with_cancel(&self, token: CancelToken) -> Self {
        Self {
            deadline: self.deadline,
            cancel: Some(token),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|t| t.is_cancelled()) || self.is_expired()
    }

    /// Time left before the deadline, capped at `default`.
    pub fn remaining(&self, default: Duration) -> Duration {
        if self.is_cancelled() {
            return Duration::ZERO;
        }
        match self.deadline {
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .min(default),
            None => default,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.deadline.map(|d| Instant::now() >= d).unwrap_or(false)
    }
}

/// Executes a command and returns its output. `Err` carries whatever text the
/// command produced (empty for output-only runners, per the Go behaviour).
pub trait Runner: Send + Sync {
    fn run(&self, ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String>;
}

/// Stdout-only runner: mirrors `exec.CommandContext(...).Output()`.
#[derive(Clone, Debug)]
pub struct OutputRunner {
    pub timeout: Duration,
}

impl Default for OutputRunner {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3),
        }
    }
}

impl Runner for OutputRunner {
    fn run(&self, ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String> {
        run_process(ctx, name, args, self.timeout, false)
    }
}

/// Combined stdout+stderr runner: mirrors `cmd.CombinedOutput()`.
#[derive(Clone, Debug)]
pub struct CombinedRunner {
    pub timeout: Duration,
}

impl Default for CombinedRunner {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
        }
    }
}

impl Runner for CombinedRunner {
    fn run(&self, ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String> {
        run_process(ctx, name, args, self.timeout, true)
    }
}

/// Adapts a closure to [`Runner`]. Used by every unit-test stub.
pub struct FnRunner<F>(pub F)
where
    F: Fn(&Ctx, &str, &[&str]) -> Result<String, String> + Send + Sync;

impl<F> Runner for FnRunner<F>
where
    F: Fn(&Ctx, &str, &[&str]) -> Result<String, String> + Send + Sync,
{
    fn run(&self, ctx: &Ctx, name: &str, args: &[&str]) -> Result<String, String> {
        (self.0)(ctx, name, args)
    }
}

/// Drains a pipe on a helper thread so a chatty child can never block on a
/// full pipe buffer while we are busy polling `try_wait`.
fn drain<R: Read + Send + 'static>(mut r: R) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut out = String::new();
        let _ = r.read_to_string(&mut out);
        out
    })
}

/// Spawn `name` with `args`, wait at most the effective timeout, then return
/// the trimmed output. Kill the child if it overruns the deadline **or** if
/// the context is cancelled (e.g. daemon shutdown during a Wi-Fi scan).
pub fn run_process(
    ctx: &Ctx,
    name: &str,
    args: &[&str],
    default_timeout: Duration,
    combined: bool,
) -> Result<String, String> {
    if ctx.is_cancelled() {
        return Err(String::new());
    }
    let timeout = ctx.remaining(default_timeout);
    let mut cmd = Command::new(name);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(if combined {
            Stdio::piped()
        } else {
            Stdio::null()
        });

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Err(format!("{}: {}", name, e)),
    };

    let out_thread = child.stdout.take().map(drain);
    let err_thread = child.stderr.take().map(drain);

    let deadline = Instant::now() + timeout;
    let mut stopped_early = false;
    let mut status = None;
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => {
                if ctx.is_cancelled() || Instant::now() >= deadline {
                    stopped_early = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break,
        }
    }

    let out = out_thread.and_then(|h| h.join().ok()).unwrap_or_default();
    let err = err_thread.and_then(|h| h.join().ok()).unwrap_or_default();
    let mut text = out;
    text.push_str(&err);

    // Go's helpers use strings.TrimRight(out, "\n"). Keep that exact shape.
    let text = text.trim_end_matches('\n').to_string();

    if stopped_early {
        return Err(text);
    }
    if status.map(|s| s.success()).unwrap_or(false) {
        Ok(text)
    } else {
        Err(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctx_clamps_to_parent() {
        let parent = Ctx::new().with_timeout(Duration::from_millis(50));
        let child = parent.with_timeout(Duration::from_secs(10));
        assert!(child.remaining(Duration::from_secs(60)) <= Duration::from_millis(50));
    }

    #[test]
    fn runs_echo_and_reports_failure() {
        let r = OutputRunner::default();
        assert_eq!(r.run(&Ctx::new(), "echo", &["hi\n"]), Ok("hi".to_string()));
        assert!(r.run(&Ctx::new(), "false", &[]).is_err());
    }

    #[test]
    fn cancel_kills_in_flight_child() {
        let token = CancelToken::new();
        let ctx = Ctx::new().with_cancel(token.clone());
        let started = Instant::now();
        let handle = thread::spawn(move || OutputRunner::default().run(&ctx, "sleep", &["30"]));
        thread::sleep(Duration::from_millis(100));
        token.cancel();
        let result = handle.join().unwrap();
        assert!(result.is_err(), "cancelled command must report failure");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "cancellation took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn timeout_kills_stubborn_child() {
        let r = OutputRunner {
            timeout: Duration::from_millis(100),
        };
        let started = Instant::now();
        let result = r.run(&Ctx::new(), "sh", &["-c", "trap '' TERM; sleep 30"]);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let r = OutputRunner {
            timeout: Duration::from_secs(5),
        };
        let out = r
            .run(&Ctx::new(), "sh", &["-c", "yes hello | head -c 200000"])
            .unwrap();
        assert!(out.len() >= 200_000);
    }

    #[test]
    fn stderr_is_only_kept_by_combined_runner() {
        let script = &["-c", "echo out; echo err >&2"];
        assert_eq!(
            OutputRunner::default().run(&Ctx::new(), "sh", script),
            Ok("out".to_string())
        );
        assert_eq!(
            CombinedRunner::default().run(&Ctx::new(), "sh", script),
            Ok("out\nerr".to_string())
        );
    }
}
