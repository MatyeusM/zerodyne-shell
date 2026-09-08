package main

import (
	"context"
	"os"
	"os/signal"
	"syscall"
)

// osSignalContext returns a context cancelled on SIGINT/SIGTERM so `daemon`
// in the foreground still exits cleanly on Ctrl-C. The background daemon
// path handles signals itself in daemon.Run.
func osSignalContext() context.Context {
	ctx, cancel := context.WithCancel(context.Background())
	ch := make(chan os.Signal, 1)
	signal.Notify(ch, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-ch
		cancel()
	}()
	return ctx
}
