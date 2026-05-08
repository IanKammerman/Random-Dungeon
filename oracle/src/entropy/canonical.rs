//! Canonicalization helpers shared across sources.
//!
//! All multi-byte integers are written big-endian. Floats are converted
//! to fixed-point integers; missing values use sentinel min-values.
//! See `docs/entropy.md` for the per-source layouts.

use anyhow::{anyhow, Result};
use chrono::DateTime;

use super::DOMAIN_TAG;

/// Sentinel for a missing 16-bit integer field.
pub const SENTINEL_I16: i16 = i16::MIN;
/// Sentinel for a missing 32-bit integer field.
pub const SENTINEL_I32: i32 = i32::MIN;

/// Start a per-source canonical buffer with the domain tag and source
/// name. Every canonical layout begins with this prefix.
pub fn open(source_name: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(DOMAIN_TAG);
    buf.extend_from_slice(source_name.as_bytes());
    buf
}

/// Write a length-prefixed UTF-8 string with a u16 length. Errors if
/// the string is longer than u16::MAX bytes (no real source ID is).
pub fn write_lp_str(buf: &mut Vec<u8>, s: &str) -> Result<()> {
    let bytes = s.as_bytes();
    let len: u16 = bytes
        .len()
        .try_into()
        .map_err(|_| anyhow!("string too long for u16 length prefix: {}", bytes.len()))?;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(bytes);
    Ok(())
}

/// Write a length-prefixed byte slice with a u16 length.
pub fn write_lp_bytes(buf: &mut Vec<u8>, b: &[u8]) -> Result<()> {
    let len: u16 = b
        .len()
        .try_into()
        .map_err(|_| anyhow!("byte slice too long for u16 length prefix: {}", b.len()))?;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(b);
    Ok(())
}

pub fn write_u32_be(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn write_u64_be(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn write_i16_be(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn write_i32_be(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub fn write_i64_be(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Convert a floating-point value to a fixed-point i32 by multiplying
/// by `scale` and rounding to nearest. Returns `SENTINEL_I32` for None
/// and saturates on overflow rather than panicking.
pub fn float_to_fixed_i32(value: Option<f64>, scale: f64) -> i32 {
    match value {
        None => SENTINEL_I32,
        Some(v) if !v.is_finite() => SENTINEL_I32,
        Some(v) => {
            let scaled = (v * scale).round();
            if scaled >= i32::MAX as f64 {
                i32::MAX
            } else if scaled <= (i32::MIN as f64) + 1.0 {
                // Reserve i32::MIN as the missing-data sentinel.
                i32::MIN + 1
            } else {
                scaled as i32
            }
        }
    }
}

/// Same idea for i16 (used for humidity in per-mille).
pub fn float_to_fixed_i16(value: Option<f64>, scale: f64) -> i16 {
    match value {
        None => SENTINEL_I16,
        Some(v) if !v.is_finite() => SENTINEL_I16,
        Some(v) => {
            let scaled = (v * scale).round();
            if scaled >= i16::MAX as f64 {
                i16::MAX
            } else if scaled <= (i16::MIN as f64) + 1.0 {
                i16::MIN + 1
            } else {
                scaled as i16
            }
        }
    }
}

/// Parse an ISO-8601 / RFC-3339 timestamp into ms since unix epoch.
/// Returns Err on malformed input — fail loud.
pub fn parse_iso8601_ms(s: &str) -> Result<i64> {
    let dt = DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow!("invalid RFC-3339 timestamp {:?}: {}", s, e))?;
    Ok(dt.timestamp_millis())
}

/// Parse a 64-character hex string into a 32-byte array. Used for
/// Bitcoin block hashes and drand randomness. Case-insensitive.
pub fn parse_hex_32(s: &str) -> Result<[u8; 32]> {
    if s.len() != 64 {
        return Err(anyhow!("expected 64 hex chars, got {}", s.len()));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16)
            .map_err(|e| anyhow!("invalid hex at byte {}: {}", i, e))?;
    }
    Ok(out)
}

/// Parse an arbitrary-length hex string into bytes (used for drand
/// signatures, where length depends on the chain).
pub fn parse_hex_var(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(anyhow!("hex string has odd length: {}", s.len()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| anyhow!("invalid hex at byte {}: {}", i / 2, e))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_i32_basic() {
        assert_eq!(float_to_fixed_i32(Some(72.5), 1000.0), 72_500);
        assert_eq!(float_to_fixed_i32(Some(-5.123), 1000.0), -5_123);
        assert_eq!(float_to_fixed_i32(None, 1000.0), i32::MIN);
        assert_eq!(float_to_fixed_i32(Some(f64::NAN), 1000.0), i32::MIN);
    }

    #[test]
    fn fixed_i32_saturates() {
        assert_eq!(float_to_fixed_i32(Some(1e20), 1.0), i32::MAX);
        assert_eq!(float_to_fixed_i32(Some(-1e20), 1.0), i32::MIN + 1);
    }

    #[test]
    fn parse_hex_32_roundtrip() {
        let s = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let b = parse_hex_32(s).unwrap();
        assert_eq!(b[0], 0x01);
        assert_eq!(b[31], 0xef);
    }

    #[test]
    fn parse_hex_32_wrong_length() {
        assert!(parse_hex_32("abcd").is_err());
    }

    #[test]
    fn iso8601_parses() {
        let ms = parse_iso8601_ms("2026-05-08T14:30:00+00:00").unwrap();
        assert!(ms > 1_700_000_000_000);
    }
}
