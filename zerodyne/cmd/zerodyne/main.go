// Command zerodyne is both the CLI client and the daemon executable.
//
//	zerodyne                  lazy-start the daemon if needed
//	zerodyne status           daemon info (lazy-starts first)
//	zerodyne status <target>  monitor snapshot: network|performance|system
//	zerodyne exec <op> [args] run an interaction, e.g. exec wifi-band 5
//	zerodyne daemon           run the daemon in the foreground
//	zerodyne stop             stop the running daemon
//
// Status/exec are thin IPC clients: they lazy-start the daemon, then send
// one JSON request over the Unix socket and print the typed response.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"zerodyne/internal/daemon"
	"zerodyne/internal/ipc"
)

func main() {
	if len(os.Args) < 2 {
		started, err := daemon.EnsureRunning("")
		if err != nil {
			fatal("lazy start: %v", err)
		}
		if started {
			fmt.Println("zerodyne daemon started")
		} else {
			fmt.Println("zerodyne daemon already running")
		}
		return
	}
	switch os.Args[1] {
	case "daemon":
		runDaemon(os.Args[2:])
	case "status":
		runStatus(os.Args[2:])
	case "exec":
		runExec(os.Args[2:])
	case "stop":
		if err := daemon.Stop(); err != nil {
			fmt.Fprintf(os.Stderr, "stop: %v\n", err)
			os.Exit(1)
		}
		fmt.Println("daemon stopped")
	default:
		fmt.Fprintf(os.Stderr, "unknown command %q\n", os.Args[1])
		usage()
		os.Exit(2)
	}
}

func usage() {
	fmt.Fprintf(os.Stderr, `Usage: %s [daemon|status|exec|stop]
  (no args)              lazy-start daemon if needed
  daemon                 run daemon in foreground
  status [target]        daemon info or monitor snapshot (network|performance|system)
  exec <operation> ...   run interaction (e.g. exec wifi-band 5)
  stop                   stop daemon
`, os.Args[0])
}

func runDaemon(args []string) {
	fs := flag.NewFlagSet("daemon", flag.ExitOnError)
	idle := fs.Duration("idle-timeout", daemon.DefaultIdleTimeout, "exit after this long without requests")
	if err := fs.Parse(args); err != nil {
		os.Exit(2)
	}
	d := daemon.New()
	if *idle > 0 {
		d.IdleTimeout = *idle
	}
	if err := d.Run(osSignalContext()); err != nil {
		fatal("daemon: %v", err)
	}
}

func runStatus(args []string) {
	target := ""
	if len(args) > 1 {
		fmt.Fprintln(os.Stderr, "usage: zerodyne status [target]")
		os.Exit(2)
	}
	if len(args) == 1 {
		target = args[0]
	}
	resp, err := daemon.Query(ipc.Request{Op: ipc.OpStatus, Target: target})
	if err != nil {
		fatal("status: %v", err)
	}
	if !resp.OK {
		fatal("status: %s", resp.Error)
	}
	// Every status reply is JSON — daemon info included — so QML can
	// JSON.parse stdout directly without a text format per target.
	printJSON(resp.Data)
}

func runExec(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "usage: zerodyne exec <operation> [arguments...]")
		os.Exit(2)
	}
	resp, err := daemon.Query(ipc.Request{Op: ipc.OpExec, Target: args[0], Args: args[1:]})
	if err != nil {
		fatal("exec: %v", err)
	}
	if !resp.OK {
		fatal("exec: %s", resp.Error)
	}
	if len(resp.Data) == 0 || string(resp.Data) == "null" {
		fmt.Println("ok")
		return
	}
	printJSON(resp.Data)
}

func printJSON(raw json.RawMessage) {
	var v any
	if err := json.Unmarshal(raw, &v); err != nil {
		fmt.Println(string(raw))
		return
	}
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	_ = enc.Encode(v)
}

func fatal(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
