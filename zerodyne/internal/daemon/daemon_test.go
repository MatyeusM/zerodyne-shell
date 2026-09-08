package daemon

import (
	"context"
	"testing"
	"time"

	"zerodyne/internal/interaction"
	"zerodyne/internal/ipc"
	"zerodyne/internal/monitor"
)

type stubMon struct {
	monitor.Base
	name string
}

func (s stubMon) Name() string                          { return s.name }
func (s stubMon) Snapshot(context.Context) (any, error) { return map[string]string{"mon": s.name}, nil }

type stubHandler struct{ name string }

func (s stubHandler) Name() string  { return s.name }
func (s stubHandler) Usage() string { return s.name }
func (s stubHandler) Exec(_ context.Context, args []string) (interaction.Result, error) {
	return interaction.Result{Message: "ran with " + string(rune('0'+len(args))) + " args"}, nil
}

func testDaemon() *Daemon {
	d := &Daemon{
		SockPath:     "test.sock",
		PidPath:      "test.pid",
		IdleTimeout:  time.Minute,
		PollEvery:    time.Second,
		Monitors:     monitor.NewRegistry(),
		Interactions: interaction.NewRegistry(),
	}
	d.Monitors.MustRegister(stubMon{name: "demo"})
	d.Interactions.MustRegister(stubHandler{name: "do"})
	d.touch()
	return d
}

func TestHandlePing(t *testing.T) {
	d := testDaemon()
	resp := d.Handle(context.Background(), ipc.Request{Op: ipc.OpPing}, make(chan struct{}, 1))
	if !resp.OK {
		t.Fatalf("ping: %+v", resp)
	}
}

func TestHandleStatusDispatch(t *testing.T) {
	d := testDaemon()
	stopCh := make(chan struct{}, 1)
	resp := d.Handle(context.Background(), ipc.Request{Op: ipc.OpStatus, Target: "demo"}, stopCh)
	if !resp.OK {
		t.Fatalf("status: %+v", resp)
	}
	var data map[string]string
	if err := resp.DecodeData(&data); err != nil || data["mon"] != "demo" {
		t.Fatalf("data = %v, %v", data, err)
	}
	resp = d.Handle(context.Background(), ipc.Request{Op: ipc.OpStatus, Target: "nope"}, stopCh)
	if resp.OK {
		t.Fatal("expected unknown-monitor error")
	}
}

func TestHandleStatusInfo(t *testing.T) {
	d := testDaemon()
	resp := d.Handle(context.Background(), ipc.Request{Op: ipc.OpStatus}, make(chan struct{}, 1))
	if !resp.OK {
		t.Fatalf("info: %+v", resp)
	}
	var info Info
	if err := resp.DecodeData(&info); err != nil {
		t.Fatal(err)
	}
	if !info.Running || len(info.Monitors) != 1 || len(info.Operations) != 1 {
		t.Fatalf("info = %+v", info)
	}
}

func TestHandleExecDispatch(t *testing.T) {
	d := testDaemon()
	stopCh := make(chan struct{}, 1)
	resp := d.Handle(context.Background(), ipc.Request{Op: ipc.OpExec, Target: "do", Args: []string{"a"}}, stopCh)
	if !resp.OK {
		t.Fatalf("exec: %+v", resp)
	}
	resp = d.Handle(context.Background(), ipc.Request{Op: ipc.OpExec, Target: "nope"}, stopCh)
	if resp.OK {
		t.Fatal("expected unknown-operation error")
	}
	resp = d.Handle(context.Background(), ipc.Request{Op: ipc.OpExec}, stopCh)
	if resp.OK {
		t.Fatal("expected missing-operation error")
	}
}

func TestHandleStop(t *testing.T) {
	d := testDaemon()
	stopCh := make(chan struct{}, 1)
	resp := d.Handle(context.Background(), ipc.Request{Op: ipc.OpStop}, stopCh)
	if !resp.OK {
		t.Fatalf("stop: %+v", resp)
	}
	select {
	case <-stopCh:
	default:
		t.Fatal("stop did not signal shutdown")
	}
}

func TestHandleUnknownOp(t *testing.T) {
	d := testDaemon()
	resp := d.Handle(context.Background(), ipc.Request{Op: "bogus"}, make(chan struct{}, 1))
	if resp.OK {
		t.Fatal("expected unknown-op error")
	}
}

func TestIdleExpiry(t *testing.T) {
	d := testDaemon()
	d.IdleTimeout = 50 * time.Millisecond
	time.Sleep(60 * time.Millisecond)
	if !d.idleExpired() {
		t.Fatal("expected idle expiry")
	}
	// Fresh activity defers it.
	d.touch()
	if d.idleExpired() {
		t.Fatal("touch should defer expiry")
	}
	// Active subscriptions hold the daemon even past the timeout.
	d.AddSub()
	d.lastSeen.Store(time.Now().Add(-time.Hour).UnixNano())
	if d.idleExpired() {
		t.Fatal("subscriptions should hold the daemon")
	}
	d.RemoveSub()
	// RemoveSub touches (unsubscribing is activity); re-age to check the
	// post-subscription state expires again.
	d.lastSeen.Store(time.Now().Add(-time.Hour).UnixNano())
	if !d.idleExpired() {
		t.Fatal("expected expiry after unsubscribing")
	}
}
