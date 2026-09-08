package network

import (
	"context"
	"strings"
	"testing"
)

func TestEscapeQrValue(t *testing.T) {
	if got := escapeQrValue(`a;b,c:d\e`); got != `a\;b\,c\:d\\e` {
		t.Fatalf("escaped %q", got)
	}
}

func TestCollapseQrMatrix(t *testing.T) {
	rows := collapseQrMatrix("##  \n  ##\n\n")
	if len(rows) != 2 || rows[0] != "10" || rows[1] != "01" {
		t.Fatalf("collapsed %+v", rows)
	}
}

func TestBuildQrClassifiesAndEncodes(t *testing.T) {
	var payload string
	run := func(_ context.Context, _ string, args ...string) (string, error) {
		payload = args[len(args)-1]
		return "##\n##\n", nil
	}
	data, err := buildQr(context.Background(), run, "wlan0", qrFields{
		ssid: "Ho;me", keyMgmt: "wpa-psk", password: "pw", hidden: "no",
	})
	if err != nil {
		t.Fatalf("build: %v", err)
	}
	if !strings.HasPrefix(payload, "WIFI:T:WPA;S:Ho\\;me;P:pw;") || !strings.HasSuffix(payload, ";;") {
		t.Fatalf("payload %q", payload)
	}
	if data.Security != "WPA" || data.Password != "pw" || data.Size != 2 {
		t.Fatalf("data %+v", data)
	}
}

func TestBuildQrRejectsEnterprise(t *testing.T) {
	run := func(_ context.Context, _ string, _ ...string) (string, error) { return "", nil }
	_, err := buildQr(context.Background(), run, "wlan0", qrFields{ssid: "Corp", keyMgmt: "wpa-eap"})
	if err == nil || !strings.Contains(err.Error(), "enterprise") {
		t.Fatalf("want enterprise error, got %v", err)
	}
}
