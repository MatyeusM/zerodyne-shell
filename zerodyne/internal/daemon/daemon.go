// Package daemon owns the background process lifecycle: single-instance
// locking, Unix socket serving, request dispatch to monitors/interactions,
// and idle shutdown.
//
// Idle shutdown: the daemon exits after IdleTimeout without requests. Every
// accepted connection counts as activity; future subscription-style clients
// hold Subs while interested. The timeout lives here — never in shell or
// QML — and is a plain field so tests can shrink it.
package daemon

import (
	"context"
	"fmt"
	"log"
	"net"
	"os"
	"os/signal"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"zerodyne/internal/caps"
	"zerodyne/internal/interaction"
	audioint "zerodyne/internal/interaction/audio"
	btint "zerodyne/internal/interaction/bluetooth"
	netint "zerodyne/internal/interaction/network"
	ppint "zerodyne/internal/interaction/powerprofile"
	powerint "zerodyne/internal/interaction/power"
	"zerodyne/internal/ipc"
	"zerodyne/internal/monitor"
	audiomon "zerodyne/internal/monitor/audio"
	gpumon "zerodyne/internal/monitor/gpu"
	ppmon "zerodyne/internal/monitor/powerprofile"
	netmon "zerodyne/internal/monitor/network"
	"zerodyne/internal/monitor/performance"
	"zerodyne/internal/monitor/system"
	"zerodyne/internal/paths"
)

// DefaultIdleTimeout: terminate ~5 minutes after the last client activity.
const DefaultIdleTimeout = 5 * time.Minute

// DefaultPollInterval caps how often the idle watcher checks.
const DefaultPollInterval = 15 * time.Second

// Daemon is a configured background instance. Build one via New, then Run.
type Daemon struct {
	SockPath    string
	PidPath     string
	IdleTimeout time.Duration
	PollEvery   time.Duration

	Monitors     *monitor.Registry
	Interactions *interaction.Registry

	startedAt time.Time
	lastSeen  atomic.Int64 // unix nanos of last request
	subs      atomic.Int64 // active subscriptions/clients holding the daemon

	mu     sync.Mutex
	subsCh map[chan struct{}]struct{} // reserved for future push updates
}

// New builds a daemon with the default monitor/interaction wiring.
// Override SockPath/PidPath/IdleTimeout before Run as needed.
func New() *Daemon {
	d := &Daemon{
		SockPath:     paths.SockFile(),
		PidPath:      paths.PidFile(),
		IdleTimeout:  DefaultIdleTimeout,
		PollEvery:    DefaultPollInterval,
		Monitors:     monitor.NewRegistry(),
		Interactions: interaction.NewRegistry(),
		startedAt:    time.Now(),
	}
	d.Monitors.MustRegister(netmon.NewMonitor())
	d.Monitors.MustRegister(netmon.NewScanMonitor())
	d.Monitors.MustRegister(audiomon.NewMonitor())
	d.Monitors.MustRegister(gpumon.NewMonitor())
	d.Monitors.MustRegister(ppmon.NewMonitor())
	d.Monitors.MustRegister(performance.NewMonitor())
	d.Monitors.MustRegister(system.NewMonitor())
	d.Interactions.MustRegister(netint.NewHandler())
	d.Interactions.MustRegister(netint.NewScanHandler())
	d.Interactions.MustRegister(netint.NewConnectHandler())
	d.Interactions.MustRegister(netint.NewDisconnectHandler())
	d.Interactions.MustRegister(netint.NewForgetHandler())
	d.Interactions.MustRegister(netint.NewQrHandler())
	d.Interactions.MustRegister(btint.NewPowerHandler())
	d.Interactions.MustRegister(btint.NewDeviceHandler())
	d.Interactions.MustRegister(audioint.NewVolumeHandler())
	d.Interactions.MustRegister(audioint.NewInputMuteHandler())
	d.Interactions.MustRegister(audioint.NewOutputDefaultHandler())
	d.Interactions.MustRegister(audioint.NewInputDefaultHandler())
	d.Interactions.MustRegister(audioint.NewOutputSwitchHandler())
	d.Interactions.MustRegister(powerint.NewHandler())
	d.Interactions.MustRegister(ppint.NewSetHandler())
	d.touch()
	return d
}

// touch records client activity, deferring idle shutdown.
func (d *Daemon) touch() {
	d.lastSeen.Store(time.Now().UnixNano())
}

// IdleFor reports how long since the last request.
func (d *Daemon) IdleFor() time.Duration {
	return time.Since(time.Unix(0, d.lastSeen.Load()))
}

// AddSub / RemoveSub bracket a client subscription that must keep the
// daemon alive past IdleTimeout. No monitor holds one yet; the counter and
// the check in idleExpired exist so the first push monitor needs no daemon
// changes.
func (d *Daemon) AddSub()    { d.subs.Add(1); d.touch() }
func (d *Daemon) RemoveSub() { d.subs.Add(-1); d.touch() }

func (d *Daemon) idleExpired() bool {
	if d.subs.Load() > 0 {
		return false
	}
	return d.IdleFor() >= d.IdleTimeout
}

// Info is the `zerodyne status` (no target) payload. Always JSON: QML
// reads monitors/operations once and capabilities to render "unavailable"
// states without polling dead backends.
type Info struct {
	Running      bool            `json:"running"`
	Pid          int             `json:"pid"`
	Sock         string          `json:"sock"`
	UptimeSecs   int64           `json:"uptime_secs"`
	IdleSecs     int64           `json:"idle_secs"`
	IdleTimeout  string          `json:"idle_timeout"`
	Monitors     []string        `json:"monitors"`
	Operations   []string        `json:"operations"`
	Capabilities map[string]bool `json:"capabilities"`
}

// Handle serves one request. It is the shared dispatch used by the socket
// loop and by tests; stopCh is closed when a stop request asks for exit.
func (d *Daemon) Handle(ctx context.Context, req ipc.Request, stopCh chan<- struct{}) ipc.Response {
	d.touch()
	switch req.Op {
	case ipc.OpPing:
		return ipc.OKResponse(map[string]any{"pong": true})
	case ipc.OpStop:
		select {
		case stopCh <- struct{}{}:
		default:
		}
		return ipc.OKResponse(map[string]any{"stopping": true})
	case ipc.OpStatus:
		if req.Target == "" {
			return ipc.OKResponse(d.info())
		}
		data, err := d.Monitors.Snapshot(ctx, req.Target)
		if err != nil {
			return ipc.ErrorResponse("%v", err)
		}
		return ipc.OKResponse(data)
	case ipc.OpExec:
		if req.Target == "" {
			return ipc.ErrorResponse("exec requires an operation (known: %s)", d.Interactions.Names())
		}
		res, err := d.Interactions.Exec(ctx, req.Target, req.Args)
		if err != nil {
			return ipc.ErrorResponse("%v", err)
		}
		return ipc.OKResponse(res)
	default:
		return ipc.ErrorResponse("unknown op %q (want status|exec|ping|stop)", req.Op)
	}
}

func (d *Daemon) info() Info {
	return Info{
		Running:      true,
		Pid:          os.Getpid(),
		Sock:         d.SockPath,
		UptimeSecs:   int64(time.Since(d.startedAt) / time.Second),
		IdleSecs:     int64(d.IdleFor() / time.Second),
		IdleTimeout:  d.IdleTimeout.String(),
		Monitors:     d.Monitors.Names(),
		Operations:   d.Interactions.Names(),
		Capabilities: caps.Probe(),
	}
}

// Run serves forever until a signal, a stop request, or idle expiry.
// It holds an flock on the pidfile (single instance), owns the socket
// path, and removes both on exit.
func (d *Daemon) Run(ctx context.Context) error {
	if err := os.MkdirAll(dirOf(d.SockPath), 0o755); err != nil {
		return fmt.Errorf("daemon: mkdir: %w", err)
	}
	lock, err := lockPidFile(d.PidPath)
	if err != nil {
		return err
	}
	defer func() { _ = lock.Close() }()

	_ = os.Remove(d.SockPath)
	l, err := net.Listen("unix", d.SockPath)
	if err != nil {
		return fmt.Errorf("daemon: listen %s: %w", d.SockPath, err)
	}
	defer func() {
		_ = l.Close()
		_ = os.Remove(d.SockPath)
		_ = os.Remove(d.PidPath)
	}()
	_ = os.Chmod(d.SockPath, 0o600)

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM, syscall.SIGHUP)
	defer signal.Stop(sigCh)

	stopReq := make(chan struct{}, 1)

	d.Monitors.StartAll(ctx)
	defer d.Monitors.StopAll()

	log.Printf("zerodyne daemon pid=%d sock=%s idle=%s", os.Getpid(), d.SockPath, d.IdleTimeout)

	go d.acceptLoop(l, stopReq)
	go d.idleLoop(stopReq)

	select {
	case sig := <-sigCh:
		log.Printf("daemon received %s, shutting down", sig)
	case <-stopReq:
		log.Printf("daemon stopping on request")
	case <-ctx.Done():
		log.Printf("daemon context cancelled, shutting down")
	}
	return nil
}

func (d *Daemon) acceptLoop(l net.Listener, stopReq chan<- struct{}) {
	for {
		conn, err := l.Accept()
		if err != nil {
			return // listener closed during shutdown
		}
		go func(c net.Conn) {
			defer c.Close()
			req, err := ipc.ReadRequest(c)
			if err != nil {
				_ = ipc.WriteResponse(c, ipc.ErrorResponse("%v", err))
				return
			}
			ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
			defer cancel()
			_ = ipc.WriteResponse(c, d.Handle(ctx, req, stopReq))
		}(conn)
	}
}

// idleLoop asks for shutdown once the daemon has been request-free (and
// subscription-free) for IdleTimeout.
func (d *Daemon) idleLoop(stopReq chan<- struct{}) {
	every := d.PollEvery
	if every <= 0 || every > time.Minute {
		every = DefaultPollInterval
	}
	t := time.NewTicker(every)
	defer t.Stop()
	for range t.C {
		if d.idleExpired() {
			log.Printf("daemon idle for %s, shutting down", d.IdleFor().Round(time.Second))
			select {
			case stopReq <- struct{}{}:
			default:
			}
			return
		}
	}
}
