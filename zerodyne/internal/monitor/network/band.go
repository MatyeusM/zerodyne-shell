package network

import (
	"strconv"
	"strings"
)

// Band helpers ported from services/network-band.sh and kept consistent
// with NetworkModel.js formatHeaderFreq: the panel label and any band
// command must agree on which frequency belongs to which band.

// Wi-Fi bands selectable via NetworkManager.
const (
	BandAuto = "auto"
	Band24   = "2.4"
	Band5    = "5"
	Band6    = "6"
)

// NmBandFor maps a user-facing band to the NetworkManager 802-11-wireless.
// band value. NM 1.44+ accepts "6GHz" so 5 and 6 GHz pin apart.
func NmBandFor(band string) (string, bool) {
	switch band {
	case Band24:
		return "bg", true
	case Band5:
		return "a", true
	case Band6:
		return "6GHz", true
	}
	return "", false
}

// BandFromNm maps a NetworkManager band value back to the user-facing
// label; anything unrecognized means "auto".
func BandFromNm(nm string) string {
	switch strings.TrimSpace(nm) {
	case "bg":
		return Band24
	case "a":
		return Band5
	case "6GHz":
		return Band6
	}
	return BandAuto
}

// BandForFreq classifies a frequency in MHz. It accepts both nmcli's
// "2412 MHz" and iw's "5745.0" forms by keeping the leading digits.
func BandForFreq(freq string) (string, bool) {
	digits := ""
	for _, r := range freq {
		if r < '0' || r > '9' {
			break
		}
		digits += string(r)
	}
	if digits == "" {
		return "", false
	}
	mhz, err := strconv.Atoi(digits)
	if err != nil {
		return "", false
	}
	switch {
	case mhz >= 2400 && mhz < 2500:
		return Band24, true
	case mhz >= 4900 && mhz < 5925:
		return Band5, true
	case mhz >= 5925 && mhz < 7125:
		return Band6, true
	}
	return "", false
}

// ValidBand reports whether s is a settable band (auto|2.4|5|6).
func ValidBand(s string) bool {
	switch s {
	case BandAuto, Band24, Band5, Band6:
		return true
	}
	return false
}

// NormalizeBandTarget accepts either a band label ("5") or a frequency in
// MHz ("5220", as in `zerodyne exec wifi-band 5220`) and returns the band.
func NormalizeBandTarget(arg string) (string, bool) {
	if ValidBand(arg) {
		return arg, true
	}
	if b, ok := BandForFreq(arg); ok {
		return b, true
	}
	return "", false
}
