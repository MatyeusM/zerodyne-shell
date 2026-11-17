//! Background process lifecycle: pidfile locking, Unix socket serving,
//! request dispatch to monitors/interactions, and idle shutdown.
//!
//! Every CLI request goes through [`ensure_running`] first:
//!
//! ```text
//! zerodyne <request>
//!     ├── daemon answers → send over IPC
//!     └── daemon missing → spawn detached daemon → wait → send
//! ```

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::caps;
use crate::interaction::Registry as Interactions;
use crate::ipc::{self, IO_TIMEOUT, OP_EXEC, OP_PING, OP_STATUS, OP_STOP, Request, Response};
use crate::monitor::Registry as Monitors;
use crate::paths;
use crate::runner::Ctx;

/// Exit ~5 minutes after the last client activity.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Cap how long the idle watcher waits between checks.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(15);
/// Hard cap on one request's context (matches the Go accept loop).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Go's goroutines are cheap; OS threads are not. Cap simultaneous clients.
const MAX_CONNECTIONS: usize = 64;
const CONNECTION_STACK: usize = 128 * 1024;

// ---- signal plumbing -----------------------------------------------------
// A self-pipe turns an async-signal-safe write into a normal poll wakeup.
static SIGNAL_FD: AtomicI32 = AtomicI32::new(-1);
static SIGNAL_SEEN: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: i32) {
    SIGNAL_SEEN.store(true, Ordering::SeqCst);
    let fd = SIGNAL_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        let byte = [1u8];
        unsafe {
            libc::write(fd, byte.as_ptr() as *const libc::c_void, 1);
        }
    }
}

fn install_signals() -> Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1i32; 2];
    // SAFETY: pipe2 writes two valid fds into `fds` or returns -1.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        return Err(anyhow!("daemon: pipe2 failed"));
    }
    // SAFETY: fds from a successful pipe2 are owned by us now.
    let read_fd = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    SIGNAL_FD.store(write_fd.as_raw_fd(), Ordering::SeqCst);
    SIGNAL_SEEN.store(false, Ordering::SeqCst);
    // SAFETY: on_signal is a plain extern "C" fn with no captured state.
    unsafe {
        let handler = on_signal as *const () as usize;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGHUP, handler);
    }
    Ok((read_fd, write_fd))
}

// ---- pidfile lock --------------------------------------------------------
struct PidLock {
    // Held only to keep the flock alive for the daemon's lifetime.
    _file: File,
    path: PathBuf,
}

impl Drop for PidLock {
    fn drop(&mut self) {
        // Only remove the pidfile if it still points at us; a newer daemon
        // may have replaced it after our lock was (unexpectedly) lost.
        let mine = std::process::id();
        if let Ok(content) = std::fs::read_to_string(&self.path)
            && content.trim().parse::<u32>().ok() == Some(mine)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn lock_pid_file(path: &Path) -> Result<PidLock> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    // SAFETY: file is open and we hold it for the daemon's lifetime.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        return match read_pid(path) {
            Ok(pid) => Err(anyhow!("daemon already running (pid {})", pid)),
            Err(_) => Err(anyhow!("daemon already running (pidfile locked)")),
        };
    }
    let mut file = file;
    file.set_len(0)?;
    file.write_all(format!("{}\n", std::process::id()).as_bytes())?;
    file.flush()?;
    Ok(PidLock {
        _file: file,
        path: path.to_path_buf(),
    })
}

fn read_pid(path: &Path) -> Result<u32> {
    let content = std::fs::read_to_string(path)?;
    content
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("invalid pidfile"))
}

fn is_running(pid: u32) -> bool {
    pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0
}

// ---- daemon --------------------------------------------------------------
pub struct Daemon {
    pub sock_path: PathBuf,
    pub pid_path: PathBuf,
    pub idle_timeout: Duration,
    pub poll_every: Duration,
    pub monitors: Arc<Monitors>,
    pub interactions: Arc<Interactions>,

    started_at: Instant,
    last_seen: AtomicU64, // unix nanos of last request
    subs: AtomicI64,      // active subscriptions holding the daemon
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

impl Daemon {
    /// Build a daemon with the default monitor/interaction wiring.
    pub fn new() -> Self {
        let d = Self {
            sock_path: paths::sock_file(),
            pid_path: paths::pid_file(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            poll_every: DEFAULT_POLL_INTERVAL,
            monitors: Arc::new(Monitors::new()),
            interactions: Arc::new(Interactions::new()),
            started_at: Instant::now(),
            last_seen: AtomicU64::new(now_nanos()),
            subs: AtomicI64::new(0),
        };

        use crate::monitor::audio::Monitor as AudioMon;
        use crate::monitor::gpu::Monitor as GpuMon;
        use crate::monitor::network::{Monitor as NetMon, ScanMonitor};
        use crate::monitor::performance::Monitor as PerfMon;
        use crate::monitor::powerprofile::Monitor as PpMon;
        use crate::monitor::system::Monitor as SysMon;
        d.monitors.must_register(Arc::new(NetMon::new()));
        d.monitors.must_register(Arc::new(ScanMonitor::new()));
        d.monitors.must_register(Arc::new(AudioMon::new()));
        d.monitors.must_register(Arc::new(GpuMon::new()));
        d.monitors.must_register(Arc::new(PpMon::new()));
        d.monitors.must_register(Arc::new(PerfMon::new()));
        d.monitors.must_register(Arc::new(SysMon::new()));

        use crate::interaction::audio::{
            InputDefaultHandler, InputMuteHandler, OutputDefaultHandler, OutputSwitchHandler,
            VolumeHandler,
        };
        use crate::interaction::bluetooth::{DeviceHandler, PowerHandler as BtPowerHandler};
        use crate::interaction::network::{
            ConnectHandler, DisconnectHandler, ForgetHandler, Handler as WifiBandHandler,
            QrHandler, ScanHandler,
        };
        use crate::interaction::power::Handler as PowerHandler;
        use crate::interaction::powerprofile::SetHandler;
        d.interactions
            .must_register(Arc::new(WifiBandHandler::new()));
        d.interactions.must_register(Arc::new(ScanHandler::new()));
        d.interactions
            .must_register(Arc::new(ConnectHandler::new()));
        d.interactions
            .must_register(Arc::new(DisconnectHandler::new()));
        d.interactions.must_register(Arc::new(ForgetHandler::new()));
        d.interactions.must_register(Arc::new(QrHandler::new()));
        d.interactions
            .must_register(Arc::new(BtPowerHandler::new()));
        d.interactions.must_register(Arc::new(DeviceHandler::new()));
        d.interactions.must_register(Arc::new(VolumeHandler::new()));
        d.interactions
            .must_register(Arc::new(InputMuteHandler::new()));
        d.interactions
            .must_register(Arc::new(OutputDefaultHandler::new()));
        d.interactions
            .must_register(Arc::new(InputDefaultHandler::new()));
        d.interactions
            .must_register(Arc::new(OutputSwitchHandler::new()));
        d.interactions.must_register(Arc::new(PowerHandler::new()));
        d.interactions.must_register(Arc::new(SetHandler::new()));
        d
    }

    /// A minimal daemon for dispatch tests (no real system access).
    #[cfg(test)]
    fn test_daemon() -> Self {
        Self {
            sock_path: PathBuf::from("test.sock"),
            pid_path: PathBuf::from("test.pid"),
            idle_timeout: Duration::from_secs(60),
            poll_every: Duration::from_secs(1),
            monitors: Arc::new(Monitors::new()),
            interactions: Arc::new(Interactions::new()),
            started_at: Instant::now(),
            last_seen: AtomicU64::new(now_nanos()),
            subs: AtomicI64::new(0),
        }
    }

    fn touch(&self) {
        self.last_seen.store(now_nanos(), Ordering::SeqCst);
    }

    pub fn idle_for(&self) -> Duration {
        let last = self.last_seen.load(Ordering::SeqCst);
        let now = now_nanos();
        Duration::from_nanos(now.saturating_sub(last))
    }

    pub fn add_sub(&self) {
        self.subs.fetch_add(1, Ordering::SeqCst);
        self.touch();
    }

    pub fn remove_sub(&self) {
        self.subs.fetch_sub(1, Ordering::SeqCst);
        self.touch();
    }

    fn idle_expired(&self) -> bool {
        if self.subs.load(Ordering::SeqCst) > 0 {
            return false;
        }
        self.idle_for() >= self.idle_timeout
    }

    fn info(&self) -> Info {
        Info {
            running: true,
            pid: std::process::id() as i64,
            sock: self.sock_path.display().to_string(),
            uptime_secs: self.started_at.elapsed().as_secs() as i64,
            idle_secs: self.idle_for().as_secs() as i64,
            idle_timeout: go_duration(self.idle_timeout),
            monitors: self.monitors.names(),
            operations: self.interactions.names(),
            capabilities: caps::probe_all(),
        }
    }

    /// Serve one request. Shared by the socket loop and tests.
    pub fn handle(&self, ctx: &Ctx, req: &Request, stop: &AtomicBool) -> Response {
        self.touch();
        match req.op.as_str() {
            OP_PING => Response::ok(json!({"pong": true})),
            OP_STOP => {
                stop.store(true, Ordering::SeqCst);
                Response::ok(json!({"stopping": true}))
            }
            OP_STATUS => {
                let target = req.target.as_deref().unwrap_or("");
                if target.is_empty() {
                    Response::ok(self.info())
                } else {
                    match self.monitors.snapshot(ctx, target) {
                        Ok(data) => Response::ok(data),
                        Err(e) => Response::error(e.to_string()),
                    }
                }
            }
            OP_EXEC => {
                let Some(target) = req.target.as_deref().filter(|s| !s.is_empty()) else {
                    return Response::error(format!(
                        "exec requires an operation (known: {})",
                        crate::go_list(&self.interactions.names())
                    ));
                };
                match self.interactions.exec(ctx, target, &req.args) {
                    Ok(res) => Response::ok(res),
                    Err(e) => Response::error(e.to_string()),
                }
            }
            other => Response::error(format!(
                "unknown op {:?} (want status|exec|ping|stop)",
                other
            )),
        }
    }

    /// Serve forever until a signal, a stop request, or idle expiry.
    pub fn run(self: Arc<Self>) -> Result<()> {
        if let Some(parent) = self.sock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _lock = lock_pid_file(&self.pid_path)?;

        let _ = std::fs::remove_file(&self.sock_path);
        let listener = UnixListener::bind(&self.sock_path)
            .map_err(|e| anyhow!("daemon: listen {}: {}", self.sock_path.display(), e))?;
        listener.set_nonblocking(true)?;
        // The closing guard removes socket and pidfile on every exit path.
        let _socket_guard = SocketGuard(self.sock_path.clone());
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.sock_path, std::fs::Permissions::from_mode(0o600));

        let (signal_read, _signal_write) = install_signals()?;
        let stop = Arc::new(AtomicBool::new(false));

        self.monitors.start_all(&Ctx::new());
        let _monitor_guard = MonitorGuard(self.monitors.clone());

        eprintln!(
            "zerodyne daemon pid={} sock={} idle={}",
            std::process::id(),
            self.sock_path.display(),
            go_duration(self.idle_timeout)
        );

        // Go: if every <= 0 || every > time.Minute { every = DefaultPollInterval }.
        let poll_ms = if self.poll_every.is_zero() || self.poll_every > Duration::from_secs(60) {
            DEFAULT_POLL_INTERVAL.as_millis() as i32
        } else {
            self.poll_every.as_millis() as i32
        };
        // Wake at most 250ms after a stop request, but check idle only as
        // often as poll_every would dictate.
        let tick_ms = 250i32;
        let mut last_idle_check = Instant::now();
        let active = Arc::new(AtomicUsize::new(0));
        let mut accept_errors: u32 = 0;

        let mut fds = [
            libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: signal_read.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            // SAFETY: fds is a valid array of two pollfd entries.
            let rc = unsafe { libc::poll(fds.as_mut_ptr(), 2, tick_ms) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                break;
            }

            if fds[1].revents & libc::POLLIN != 0 {
                let mut buf = [0u8; 64];
                // SAFETY: signal_read is a valid nonblocking fd.
                unsafe {
                    libc::read(
                        signal_read.as_raw_fd(),
                        buf.as_mut_ptr() as *mut libc::c_void,
                        64,
                    );
                }
                if SIGNAL_SEEN.load(Ordering::SeqCst) {
                    eprintln!("daemon received signal, shutting down");
                    stop.store(true, Ordering::SeqCst);
                    break;
                }
            }

            if fds[0].revents & libc::POLLIN != 0 {
                loop {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            accept_errors = 0;
                            // Shed excess clients instead of spawning an
                            // unbounded number of default-stack threads.
                            if active
                                .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                                    (n < MAX_CONNECTIONS).then_some(n + 1)
                                })
                                .is_err()
                            {
                                continue;
                            }
                            let daemon = self.clone();
                            let stop = stop.clone();
                            let permits = active.clone();
                            let spawned = std::thread::Builder::new()
                                .name("zerodyne-client".into())
                                .stack_size(CONNECTION_STACK)
                                .spawn(move || {
                                    let _guard = ConnectionGuard(permits);
                                    // A client that connects and never sends
                                    // must not pin this thread forever.
                                    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
                                    let req = match ipc::read_request(&mut stream) {
                                        Ok(r) => r,
                                        Err(e) => {
                                            let _ = ipc::write_response(
                                                &mut stream,
                                                &Response::error(e.to_string()),
                                            );
                                            return;
                                        }
                                    };
                                    let ctx = Ctx::new().with_timeout(REQUEST_TIMEOUT);
                                    let resp = daemon.handle(&ctx, &req, &stop);
                                    let _ = ipc::write_response(&mut stream, &resp);
                                });
                            if spawned.is_err() {
                                active.fetch_sub(1, Ordering::SeqCst);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => {
                            // Level-triggered poll would spin on a persistent
                            // error (e.g. EMFILE); back off progressively.
                            accept_errors += 1;
                            eprintln!("daemon: accept: {}", e);
                            std::thread::sleep(Duration::from_millis(10) * accept_errors.min(10));
                            break;
                        }
                    }
                }
            }

            if last_idle_check.elapsed() >= Duration::from_millis(poll_ms as u64) {
                last_idle_check = Instant::now();
                if self.idle_expired() {
                    eprintln!(
                        "daemon idle for {}, shutting down",
                        go_duration(round_secs(self.idle_for()))
                    );
                    stop.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Owns only the socket. The pidfile belongs to `PidLock`, which checks
/// ownership before removing it.
struct SocketGuard(PathBuf);
impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Returns a connection permit when the worker thread finishes.
struct ConnectionGuard(Arc<AtomicUsize>);
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

struct MonitorGuard(Arc<Monitors>);
impl Drop for MonitorGuard {
    fn drop(&mut self) {
        self.0.stop_all();
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// `zerodyne status` (no target) payload. Every field is always present so
/// QML can render unavailable states without polling dead backends.
#[derive(Serialize)]
pub struct Info {
    running: bool,
    pid: i64,
    sock: String,
    uptime_secs: i64,
    idle_secs: i64,
    idle_timeout: String,
    monitors: Vec<String>,
    operations: Vec<String>,
    capabilities: HashMap<String, bool>,
}

// ---- lazy client helpers -------------------------------------------------
pub fn alive() -> bool {
    ipc::ping(&paths::sock_file())
}

/// Spawn a detached daemon if none answers. Returns true when one started.
pub fn ensure_running(exe: &str) -> Result<bool> {
    if alive() {
        return Ok(false);
    }
    // Drop a stale pidfile only when its owner is really gone; otherwise a
    // daemon that is still binding its socket could lose its lock file.
    if !paths::sock_file().exists()
        && let Ok(pid) = read_pid(&paths::pid_file())
        && !is_running(pid)
    {
        let _ = std::fs::remove_file(paths::pid_file());
    }
    let exe = if exe.is_empty() {
        std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from(std::env::args().next().unwrap_or_default()))
    } else {
        PathBuf::from(exe)
    };
    let mut cmd = Command::new(exe);
    cmd.arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // SAFETY: setsid in the forked child detaches the daemon from this TTY.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map_err(|e| anyhow!("start daemon: {}", e))?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if alive() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(anyhow!("daemon did not answer within 5s"))
}

fn remove_runtime_files() {
    let _ = std::fs::remove_file(paths::sock_file());
    let _ = std::fs::remove_file(paths::pid_file());
}

fn wait_exit(pid: u32, millis: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        if !is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !is_running(pid)
}

/// Ask the daemon to exit, falling back to SIGTERM/SIGKILL via the pidfile.
/// Always verifies the process is gone before returning Ok, and cleans up
/// socket/pidfile on every path (a killed daemon cannot run its own guards).
pub fn stop() -> Result<()> {
    if alive() {
        let _ = ipc::send(
            &paths::sock_file(),
            &Request {
                op: OP_STOP.to_string(),
                ..Default::default()
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if !alive() {
                remove_runtime_files();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let pid = match read_pid(&paths::pid_file()) {
        Ok(p) => p,
        Err(_) => {
            remove_runtime_files();
            return Err(anyhow!("daemon not running"));
        }
    };
    if !is_running(pid) {
        remove_runtime_files();
        return Err(anyhow!("daemon not running (stale pid {} cleaned)", pid));
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    if wait_exit(pid, 3_000) {
        remove_runtime_files();
        return Ok(());
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    if wait_exit(pid, 1_000) {
        remove_runtime_files();
        return Ok(());
    }
    Err(anyhow!("daemon pid {} did not exit after SIGKILL", pid))
}

/// Ensure the daemon is up and send one request (with one startup retry).
pub fn query(req: Request) -> Result<Response> {
    ensure_running("")?;
    match ipc::send(&paths::sock_file(), &req) {
        Ok(resp) => Ok(resp),
        Err(_) => {
            std::thread::sleep(Duration::from_millis(300));
            ipc::send(&paths::sock_file(), &req)
        }
    }
}

const MAX_NANOS: i128 = i64::MAX as i128;

fn bad_duration(orig: &str) -> anyhow::Error {
    anyhow!("time: invalid duration {:?}", orig)
}

/// Port of Go's `time.ParseDuration`: signs, fractional components, both
/// micro signs, and overflow rejection. Returns signed nanoseconds so the
/// caller can apply Go's `if idle > 0` rule for negative values.
pub fn parse_duration_nanos(input: &str) -> Result<i128> {
    let orig = input;
    let mut s = input;
    let mut neg = false;
    if let Some(rest) = s.strip_prefix('-') {
        neg = true;
        s = rest;
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest;
    }
    if s.is_empty() {
        return Err(bad_duration(orig));
    }
    if s == "0" {
        return Ok(0);
    }
    let mut total: i128 = 0;
    while !s.is_empty() {
        let bytes = s.as_bytes();
        let mut i = 0;
        let mut whole: i128 = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            whole = whole
                .checked_mul(10)
                .and_then(|v| v.checked_add((bytes[i] - b'0') as i128))
                .filter(|v| *v <= MAX_NANOS)
                .ok_or_else(|| bad_duration(orig))?;
            i += 1;
        }
        let (mut frac, mut scale, mut j) = (0i128, 1i128, i);
        if j < bytes.len() && bytes[j] == b'.' {
            j += 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                if scale < 10_i128.pow(18) {
                    frac = frac * 10 + (bytes[j] - b'0') as i128;
                    scale *= 10;
                }
                j += 1;
            }
        }
        if i == 0 && j == 0 {
            return Err(bad_duration(orig));
        }
        let mut k = j;
        while k < bytes.len() && !bytes[k].is_ascii_digit() && bytes[k] != b'.' {
            k += 1;
        }
        let unit = &s[j..k];
        let per: i128 = match unit {
            "ns" => 1,
            "us" | "µs" | "μs" => 1_000,
            "ms" => 1_000_000,
            "s" => 1_000_000_000,
            "m" => 60_000_000_000,
            "h" => 3_600_000_000_000,
            _ => {
                return Err(anyhow!(
                    "time: unknown unit {:?} in duration {:?}",
                    unit,
                    orig
                ));
            }
        };
        let mut value = whole.checked_mul(per).ok_or_else(|| bad_duration(orig))?;
        if frac > 0 {
            value = value
                .checked_add(((frac as f64 / scale as f64) * per as f64) as i128)
                .ok_or_else(|| bad_duration(orig))?;
        }
        total = total
            .checked_add(value)
            .filter(|v| *v <= MAX_NANOS)
            .ok_or_else(|| bad_duration(orig))?;
        s = &s[k..];
    }
    Ok(if neg { -total } else { total })
}

/// Parse a Go-style duration for `--idle-timeout`. Non-positive values map
/// to zero (the CLI keeps its default then), just like Go's `if *idle > 0`.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let nanos = parse_duration_nanos(s)?;
    if nanos <= 0 {
        return Ok(Duration::ZERO);
    }
    u64::try_from(nanos)
        .map(Duration::from_nanos)
        .map_err(|_| bad_duration(s))
}

/// Format nanoseconds with Go's fractional algorithm (no trailing zeros).
fn fmt_frac(v: u128, prec: u32) -> (String, u128) {
    let mut digits = Vec::with_capacity(prec as usize);
    let mut rest = v;
    for _ in 0..prec {
        digits.push((b'0' + (rest % 10) as u8) as char);
        rest /= 10;
    }
    let mut frac = String::new();
    if let Some(last) = digits.iter().rposition(|c| *c != '0') {
        frac.push('.');
        for c in digits[last..].iter().rev() {
            frac.push(*c);
        }
    }
    (frac, rest)
}

/// Render a Duration the way Go's `time.Duration.String()` does.
pub fn go_duration(d: Duration) -> String {
    let nanos = d.as_nanos();
    if nanos == 0 {
        return "0s".into();
    }
    if nanos < 1_000_000_000 {
        let (prec, unit) = if nanos < 1_000 {
            (0, "ns")
        } else if nanos < 1_000_000 {
            (3, "µs")
        } else {
            (6, "ms")
        };
        let (frac, whole) = fmt_frac(nanos, prec);
        return format!("{}{}{}", whole, frac, unit);
    }
    let (frac, total_secs) = fmt_frac(nanos, 9);
    let minutes = total_secs / 60;
    let mut out = String::new();
    if minutes >= 60 {
        out.push_str(&format!("{}h", minutes / 60));
    }
    if minutes > 0 {
        out.push_str(&format!("{}m", minutes % 60));
    }
    out.push_str(&format!("{}{}s", total_secs % 60, frac));
    out
}

fn round_secs(d: Duration) -> Duration {
    let s = d.as_secs();
    if d.subsec_nanos() >= 500_000_000 {
        Duration::from_secs(s + 1)
    } else {
        Duration::from_secs(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{Handler, ResultData};
    use crate::monitor::Monitor;

    struct StubMon;
    impl Monitor for StubMon {
        fn name(&self) -> &'static str {
            "demo"
        }
        fn snapshot(&self, _: &Ctx) -> Result<serde_json::Value> {
            Ok(json!({"mon": "demo"}))
        }
    }
    struct StubHandler;
    impl Handler for StubHandler {
        fn name(&self) -> &'static str {
            "do"
        }
        fn usage(&self) -> &'static str {
            "do"
        }
        fn exec(&self, _: &Ctx, args: &[String]) -> Result<ResultData> {
            Ok(ResultData::message(format!("ran with {} args", args.len())))
        }
    }

    fn daemon() -> Daemon {
        let d = Daemon::test_daemon();
        d.monitors.must_register(Arc::new(StubMon));
        d.interactions.must_register(Arc::new(StubHandler));
        d
    }

    #[test]
    fn ping_stop_status_exec() {
        let d = daemon();
        let stop = AtomicBool::new(false);
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: OP_PING.into(),
                    ..Default::default()
                },
                &stop
            )
            .ok
        );
        let resp = d.handle(
            &Ctx::new(),
            &Request {
                op: OP_STATUS.into(),
                target: Some("demo".into()),
                args: vec![],
            },
            &stop,
        );
        assert_eq!(resp.data.unwrap()["mon"], "demo");
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: OP_STATUS.into(),
                    target: Some("nope".into()),
                    args: vec![]
                },
                &stop
            )
            .error
            .contains("unknown status target")
        );
        let info = d.handle(
            &Ctx::new(),
            &Request {
                op: OP_STATUS.into(),
                ..Default::default()
            },
            &stop,
        );
        assert_eq!(info.data.unwrap()["running"], true);
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: OP_EXEC.into(),
                    target: Some("do".into()),
                    args: vec!["a".into()]
                },
                &stop
            )
            .ok
        );
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: OP_EXEC.into(),
                    ..Default::default()
                },
                &stop
            )
            .error
            .contains("exec requires an operation")
        );
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: OP_STOP.into(),
                    ..Default::default()
                },
                &stop
            )
            .ok
        );
        assert!(stop.load(Ordering::SeqCst));
        assert!(
            d.handle(
                &Ctx::new(),
                &Request {
                    op: "bogus".into(),
                    ..Default::default()
                },
                &stop
            )
            .error
            .contains("unknown op")
        );
    }

    #[test]
    fn idle_expiry() {
        let mut d = Daemon::test_daemon();
        d.idle_timeout = Duration::from_millis(50);
        d.last_seen
            .store(now_nanos() - 60_000_000, Ordering::SeqCst);
        assert!(d.idle_expired());
        d.touch();
        assert!(!d.idle_expired());
        d.add_sub();
        d.last_seen
            .store(now_nanos() - 3_600_000_000_000, Ordering::SeqCst);
        assert!(!d.idle_expired());
        d.remove_sub();
        d.last_seen
            .store(now_nanos() - 3_600_000_000_000, Ordering::SeqCst);
        assert!(d.idle_expired());
    }

    #[test]
    fn duration_parse_golden_table() {
        let cases: &[(&str, i128)] = &[
            ("0", 0),
            ("5m", 300_000_000_000),
            ("+5m", 300_000_000_000),
            ("-2s", -2_000_000_000),
            ("1h30m", 5_400_000_000_000),
            ("250ms", 250_000_000),
            ("1.5s", 1_500_000_000),
            ("1.0005s", 1_000_500_000),
            (".5s", 500_000_000),
            ("1us", 1_000),
            ("1µs", 1_000), // U+00B5
            ("1μs", 1_000), // U+03BC
            ("1.5ms", 1_500_000),
        ];
        for &(input, want) in cases {
            assert_eq!(
                parse_duration_nanos(input).unwrap_or_else(|e| panic!("{input}: {e}")),
                want,
                "parse {input}"
            );
        }
        for bad in [
            "",
            "1",
            "5x",
            "1.2.3s",
            "9223372036854775808ns",
            "2562047h47m16.854775808s",
        ] {
            assert!(parse_duration_nanos(bad).is_err(), "must reject {bad:?}");
        }
        // Negative values are accepted by the parser but ignored by the CLI.
        assert_eq!(parse_duration("-2s").unwrap(), Duration::ZERO);
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
    }

    #[test]
    fn duration_render_golden_table() {
        let cases: &[(Duration, &str)] = &[
            (Duration::ZERO, "0s"),
            (Duration::from_nanos(1), "1ns"),
            (Duration::from_nanos(1_500), "1.5µs"),
            (Duration::from_nanos(1_500_000), "1.5ms"),
            (Duration::from_micros(1_000_500), "1.0005s"),
            (Duration::from_millis(1_500), "1.5s"),
            (Duration::from_millis(60_500), "1m0.5s"),
            (Duration::from_secs(15), "15s"),
            (Duration::from_secs(300), "5m0s"),
            (Duration::from_secs(3600), "1h0m0s"),
        ];
        for &(d, want) in cases {
            assert_eq!(go_duration(d), want, "render {d:?}");
        }
        assert_eq!(go_duration(round_secs(Duration::from_millis(1_500))), "2s");
    }
}
