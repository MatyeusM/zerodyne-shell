//! End-to-end daemon lifecycle against a real Unix socket in an isolated
//! XDG_RUNTIME_DIR. The live user daemon is never touched.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zerodyne::ipc::{self, OP_STATUS, OP_STOP, Request};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn runtime_dir(tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("zrs-it-{}-{}-{}", std::process::id(), tag, n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn wait_for(path: &Path, millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

fn wait_removed(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if !path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("{} was not cleaned up", path.display());
}

fn start(rt: &Path) -> (std::process::Child, PathBuf, PathBuf) {
    let log = File::create(rt.join("daemon.log")).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_zerodyne"))
        .arg("daemon")
        .env("XDG_RUNTIME_DIR", rt)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .unwrap();
    let sock = rt.join("zerodyne/zerodyne.sock");
    let pid = rt.join("zerodyne/zerodyne.pid");
    wait_for(&sock, 5_000);
    (child, sock, pid)
}

fn stop(child: &mut std::process::Child, sock: &Path) -> Duration {
    let started = Instant::now();
    let resp = ipc::send(
        sock,
        &Request {
            op: OP_STOP.to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(resp.ok, "{}", resp.error);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "daemon exited with {status}");
                break;
            }
            None => {
                assert!(Instant::now() < deadline, "daemon did not exit");
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    started.elapsed()
}

#[test]
fn status_stop_and_cleanup() {
    let rt = runtime_dir("lifecycle");
    let (mut child, sock, pid) = start(&rt);

    let resp = ipc::send(
        &sock,
        &Request {
            op: OP_STATUS.to_string(),
            target: Some("system".into()),
            args: vec![],
        },
    )
    .unwrap();
    assert!(resp.ok, "{}", resp.error);
    assert!(resp.data.unwrap().get("hostname").is_some());

    let elapsed = stop(&mut child, &sock);
    assert!(
        elapsed < Duration::from_millis(2_000),
        "shutdown took {elapsed:?}"
    );
    wait_removed(&sock);
    wait_removed(&pid);
}

#[test]
fn repeated_start_stop_reuses_runtime_dir() {
    let rt = runtime_dir("repeat");
    for round in 0..2 {
        let (mut child, sock, pid) = start(&rt);
        let resp = ipc::send(
            &sock,
            &Request {
                op: OP_STATUS.to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(resp.ok, "round {round}: {}", resp.error);
        stop(&mut child, &sock);
        wait_removed(&sock);
        wait_removed(&pid);
    }
}
