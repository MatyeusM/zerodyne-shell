package bluetooth

import (
	"context"
	"strings"
	"testing"
)

const testAddr = "44:73:D6:A4:73:3F"

func btSequence(calls []btCall) string {
	var parts []string
	for _, c := range calls {
		parts = append(parts, c.name+" "+strings.Join(c.args, " "))
	}
	return strings.Join(parts, "\n")
}

func TestDeviceUsage(t *testing.T) {
	run := stubBt(true, nil)
	for _, args := range [][]string{
		nil,
		{"connect"},
		{"connect", testAddr, "extra"},
		{"explode", testAddr},
		{"connect", "not-a-mac"},
		{"connect", "44:73:D6:A4:73:3Z"},
	} {
		_, err := NewDeviceHandlerWithRunner(run).Exec(context.Background(), args)
		if err == nil || !strings.Contains(err.Error(), "usage:") {
			t.Fatalf("want usage error for %v, got %v", args, err)
		}
	}
}

func TestDevicePairSequence(t *testing.T) {
	var calls []btCall
	res, err := NewDeviceHandlerWithRunner(stubBt(true, &calls)).Exec(context.Background(), []string{"pair", testAddr})
	if err != nil || res.Message == "" {
		t.Fatalf("exec: %v %v", res, err)
	}
	seq := btSequence(calls)
	for _, want := range []string{
		"bluetoothctl show",
		"bluetoothctl pair " + testAddr,
		"bluetoothctl trust " + testAddr,
		"bluetoothctl connect " + testAddr,
	} {
		if !strings.Contains(seq, want) {
			t.Fatalf("sequence missing %q:\n%s", want, seq)
		}
	}
	if strings.Index(seq, "bluetoothctl pair") > strings.Index(seq, "bluetoothctl connect") {
		t.Fatalf("pair must precede connect:\n%s", seq)
	}
}

func TestDeviceConnectSkipsPair(t *testing.T) {
	var calls []btCall
	if _, err := NewDeviceHandlerWithRunner(stubBt(true, &calls)).Exec(context.Background(), []string{"connect", testAddr}); err != nil {
		t.Fatal(err)
	}
	seq := btSequence(calls)
	if strings.Contains(seq, "bluetoothctl pair") {
		t.Fatalf("connect must not pair:\n%s", seq)
	}
	if !strings.Contains(seq, "bluetoothctl trust "+testAddr) || !strings.Contains(seq, "bluetoothctl connect "+testAddr) {
		t.Fatalf("connect needs trust+connect:\n%s", seq)
	}
}

func TestDeviceDisconnectAlone(t *testing.T) {
	var calls []btCall
	if _, err := NewDeviceHandlerWithRunner(stubBt(false, &calls)).Exec(context.Background(), []string{"disconnect", testAddr}); err != nil {
		t.Fatal(err)
	}
	if len(calls) != 1 || btSequence(calls) != "bluetoothctl disconnect "+testAddr {
		t.Fatalf("disconnect should be a single call, got:\n%s", btSequence(calls))
	}
}

func TestDeviceForgetDisconnectsThenRemoves(t *testing.T) {
	var calls []btCall
	if _, err := NewDeviceHandlerWithRunner(stubBt(true, &calls)).Exec(context.Background(), []string{"forget", testAddr}); err != nil {
		t.Fatal(err)
	}
	seq := btSequence(calls)
	disc := strings.Index(seq, "bluetoothctl disconnect "+testAddr)
	rem := strings.Index(seq, "bluetoothctl remove "+testAddr)
	if disc < 0 || rem < 0 || disc > rem {
		t.Fatalf("forget needs disconnect then remove:\n%s", seq)
	}
}
