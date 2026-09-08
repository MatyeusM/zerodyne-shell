package network

import (
	"context"
	"errors"
	"strings"
	"testing"
)

func stubConn(handler func(name string, args []string) (string, error)) runFunc {
	return func(_ context.Context, name string, args ...string) (string, error) {
		return handler(name, args)
	}
}

func TestConnectMapsNeedPassword(t *testing.T) {
	run := stubConn(func(name string, args []string) (string, error) {
		return "Error: Secrets were required, but not provided", errors.New("exit 1")
	})
	_, err := NewConnectHandlerWithRunner(run).Exec(context.Background(), []string{"Cafe"})
	if err == nil || err.Error() != NeedPassword {
		t.Fatalf("want need-password, got %v", err)
	}
}

func TestConnectPassesPassword(t *testing.T) {
	var got []string
	run := stubConn(func(name string, args []string) (string, error) {
		got = args
		return "Connection successfully activated", nil
	})
	res, err := NewConnectHandlerWithRunner(run).Exec(context.Background(), []string{"Home", "s3cret"})
	if err != nil {
		t.Fatalf("exec: %v", err)
	}
	joined := strings.Join(got, " ")
	if !strings.Contains(joined, "password s3cret") || res.Message == "" {
		t.Fatalf("password not forwarded: %v", got)
	}
}

func TestDisconnectFindsDevice(t *testing.T) {
	var disconnected string
	run := stubConn(func(name string, args []string) (string, error) {
		if args[0] == "-e" {
			return "eth0:ethernet:connected\nwlan0:wifi:connected\n", nil
		}
		disconnected = args[len(args)-1]
		return "", nil
	})
	if _, err := NewDisconnectHandlerWithRunner(run).Exec(context.Background(), nil); err != nil {
		t.Fatalf("exec: %v", err)
	}
	if disconnected != "wlan0" {
		t.Fatalf("disconnected %q, want wlan0", disconnected)
	}
}

func TestDisconnectWithoutWifiErrors(t *testing.T) {
	run := stubConn(func(name string, args []string) (string, error) {
		return "eth0:ethernet:connected\n", nil
	})
	if _, err := NewDisconnectHandlerWithRunner(run).Exec(context.Background(), nil); err == nil {
		t.Fatal("want error without wifi")
	}
}

func TestForgetDeletesMatchingProfile(t *testing.T) {
	var deleted string
	run := stubConn(func(name string, args []string) (string, error) {
		if strings.Contains(strings.Join(args, " "), "802-11-wireless.ssid") {
			return "Home:Home\nCafe:Coffee Shop\n", nil
		}
		deleted = args[len(args)-1]
		return "", nil
	})
	if _, err := NewForgetHandlerWithRunner(run).Exec(context.Background(), []string{"Coffee Shop"}); err != nil {
		t.Fatalf("exec: %v", err)
	}
	if deleted != "Cafe" {
		t.Fatalf("deleted %q, want Cafe", deleted)
	}
}

func TestForgetUnknownErrors(t *testing.T) {
	run := stubConn(func(name string, args []string) (string, error) {
		return "Home:Home\n", nil
	})
	if _, err := NewForgetHandlerWithRunner(run).Exec(context.Background(), []string{"Nope"}); err == nil {
		t.Fatal("want error for unknown ssid")
	}
}
