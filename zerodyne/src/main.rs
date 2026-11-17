//! Command-line client and daemon executable, mirroring the Go CLI:
//!
//! ```text
//! zerodyne                  lazy-start the daemon if needed
//! zerodyne status           daemon info (lazy-starts first)
//! zerodyne status <target>  monitor snapshot: network|wifi|audio|gpu|...
//! zerodyne exec <op> [args] run an interaction, e.g. exec wifi-band 5
//! zerodyne daemon           run the daemon in the foreground
//! zerodyne stop             stop the running daemon
//! ```

use serde_json::Value;
use std::process::exit;
use std::sync::Arc;
use zerodyne::daemon;
use zerodyne::ipc::{OP_EXEC, OP_STATUS, Request};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        match daemon::ensure_running("") {
            Ok(true) => println!("zerodyne daemon started"),
            Ok(false) => println!("zerodyne daemon already running"),
            Err(e) => fatal(&format!("lazy start: {}", e)),
        }
        return;
    }
    match args[1].as_str() {
        "daemon" => run_daemon(&args[2..]),
        "status" => run_status(&args[2..]),
        "exec" => run_exec(&args[2..]),
        "stop" => match daemon::stop() {
            Ok(()) => println!("daemon stopped"),
            Err(e) => fatal(&format!("stop: {}", e)),
        },
        other => {
            eprintln!("unknown command {:?}", other);
            usage();
            exit(2);
        }
    }
}

fn usage() {
    eprintln!(
        "Usage: zerodyne [daemon|status|exec|stop]\n\
           (no args)              lazy-start daemon if needed\n\
           daemon                 run daemon in foreground\n\
           status [target]        daemon info or monitor snapshot\n\
           exec <operation> ...   run interaction (e.g. exec wifi-band 5)\n\
           stop                   stop daemon"
    );
}

fn run_daemon(args: &[String]) {
    let mut idle = daemon::DEFAULT_IDLE_TIMEOUT;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        // Go's flag package accepts both single- and double-dash forms.
        let value = if let Some(v) = arg
            .strip_prefix("--idle-timeout=")
            .or_else(|| arg.strip_prefix("-idle-timeout="))
        {
            Some(v.to_string())
        } else if arg == "--idle-timeout" || arg == "-idle-timeout" {
            i += 1;
            args.get(i).cloned()
        } else {
            eprintln!("unknown daemon flag {:?}", arg);
            exit(2);
        };
        let Some(value) = value else {
            eprintln!("daemon: --idle-timeout requires a value");
            exit(2);
        };
        match daemon::parse_duration(&value) {
            Ok(d) => {
                if !d.is_zero() {
                    idle = d;
                }
            }
            Err(e) => fatal(&format!("daemon: {}", e)),
        }
        i += 1;
    }

    let mut d = daemon::Daemon::new();
    d.idle_timeout = idle;
    if let Err(e) = Arc::new(d).run() {
        fatal(&format!("daemon: {}", e));
    }
}

fn run_status(args: &[String]) {
    if args.len() > 1 {
        eprintln!("usage: zerodyne status [target]");
        exit(2);
    }
    let target = args.first().cloned();
    let resp = match daemon::query(Request {
        op: OP_STATUS.to_string(),
        target,
        args: vec![],
    }) {
        Ok(r) => r,
        Err(e) => fatal(&format!("status: {}", e)),
    };
    if !resp.ok {
        fatal(&format!("status: {}", resp.error));
    }
    // Every status reply is JSON — QML parses stdout directly.
    print_json(resp.data);
}

fn run_exec(args: &[String]) {
    if args.is_empty() {
        eprintln!("usage: zerodyne exec <operation> [arguments...]");
        exit(2);
    }
    let resp = match daemon::query(Request {
        op: OP_EXEC.to_string(),
        target: Some(args[0].clone()),
        args: args[1..].to_vec(),
    }) {
        Ok(r) => r,
        Err(e) => fatal(&format!("exec: {}", e)),
    };
    if !resp.ok {
        fatal(&format!("exec: {}", resp.error));
    }
    match resp.data {
        None | Some(Value::Null) => println!("ok"),
        Some(data) => print_json(Some(data)),
    }
}

fn print_json(data: Option<Value>) {
    match data {
        Some(v) => match serde_json::to_string_pretty(&v) {
            Ok(s) => println!("{}", s),
            Err(_) => println!("{}", v),
        },
        None => println!("null"),
    }
}

fn fatal(message: &str) -> ! {
    eprintln!("{}", message);
    exit(1)
}
