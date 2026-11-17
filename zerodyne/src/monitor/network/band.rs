//! Wi-Fi band helpers ported from `network-band.sh` and `band.go`.
//! The panel label and band commands must agree on frequency classification.

pub const BAND_AUTO: &str = "auto";
pub const BAND_24: &str = "2.4";
pub const BAND_5: &str = "5";
pub const BAND_6: &str = "6";

/// Map a user-facing band to the NetworkManager 802-11-wireless band value.
pub fn nm_band_for(band: &str) -> Option<String> {
    match band {
        BAND_24 => Some("bg".into()),
        BAND_5 => Some("a".into()),
        BAND_6 => Some("6GHz".into()),
        _ => None,
    }
}

/// Map a NetworkManager band value back to a user-facing label.
pub fn band_from_nm(nm: &str) -> String {
    match nm.trim() {
        "bg" => BAND_24,
        "a" => BAND_5,
        "6GHz" => BAND_6,
        _ => BAND_AUTO,
    }
    .to_string()
}

/// Classify a frequency in MHz. Accepts both `2412 MHz` and `5745.0` forms
/// by keeping the leading run of ASCII digits.
pub fn band_for_freq(freq: &str) -> Option<String> {
    let digits: String = freq.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let mhz: i64 = digits.parse().ok()?;
    let band = if (2400..2500).contains(&mhz) {
        BAND_24
    } else if (4900..5925).contains(&mhz) {
        BAND_5
    } else if (5925..7125).contains(&mhz) {
        BAND_6
    } else {
        return None;
    };
    Some(band.to_string())
}

/// Whether `s` is a settable band (`auto|2.4|5|6`).
pub fn valid_band(s: &str) -> bool {
    matches!(s, BAND_AUTO | BAND_24 | BAND_5 | BAND_6)
}

/// Accept either a band label (`5`) or a frequency in MHz (`5220`).
pub fn normalize_band_target(arg: &str) -> Option<String> {
    if valid_band(arg) {
        Some(arg.to_string())
    } else {
        band_for_freq(arg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_for_freq_cases() {
        let cases = [
            ("2412 MHz", Some("2.4")),
            ("2412", Some("2.4")),
            ("5745.0", Some("5")),
            ("5180 MHz", Some("5")),
            ("6135", Some("6")),
            ("6", None),
            ("", None),
            ("abc", None),
        ];
        for (input, want) in cases {
            assert_eq!(
                band_for_freq(input).as_deref(),
                want,
                "BandForFreq({:?})",
                input
            );
        }
    }

    #[test]
    fn nm_band_round_trip() {
        for b in [BAND_24, BAND_5, BAND_6] {
            let nm = nm_band_for(b).unwrap();
            assert_eq!(band_from_nm(&nm), b);
        }
        assert_eq!(band_from_nm("whatever"), BAND_AUTO);
    }

    #[test]
    fn normalize_band_target_cases() {
        assert_eq!(normalize_band_target("5220").as_deref(), Some("5"));
        assert_eq!(normalize_band_target("auto").as_deref(), Some("auto"));
        assert_eq!(normalize_band_target("bogus"), None);
    }
}
