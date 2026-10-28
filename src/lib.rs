//! # rfc-2045-quoted-printable-decoder
//!
//! Decodes MIME quoted-printable encoded text as defined by RFC 2045,
//! Section 6.7. Handles soft line breaks ("=" at end of line) and
//! hexadecimal escape sequences ("=XX").

/// Decodes a quoted-printable encoded byte slice.
///
/// Implements RFC 2045 Section 6.7. The decoder performs these transformations:
///
/// - Soft line breaks: A line ending in "=" (followed by CRLF or LF) is
///   a soft break. The "=" and the line terminator are removed, joining
///   the next line directly. (Rule #1, §6.7, note (2))
/// - Hex escapes: "=XX" where X is a hex digit yields the byte with that
///   value. (Rule #2, §6.7)
/// - Literal text: All other printable ASCII characters (33–126) and
///   whitespace pass through unchanged. (Rule #3, §6.7)
///
/// # Edge cases and decisions
///
/// This decoder operates on raw bytes and returns raw bytes. It does not
/// attempt character-set transcoding — that is the job of a higher layer
/// per RFC 2045's transfer-encoding vs. charset separation.
///
/// Trailing whitespace (spaces/tabs) before a soft line break is preserved
/// rather than stripped. RFC 2045's advice in §6.7 note (5) recommends
/// encoders strip such whitespace, but does not mandate decoders reject it.
/// Stripping would destroy legitimate whitespace in non-soft-break lines,
/// so we preserve it.
///
/// A "=" followed by anything that is not a valid two-hex-digit sequence
/// is a malformed input. This function leaves such bytes verbatim in the
/// output rather than erroring, because real-world email often contains
/// slightly non-conformant bodies and silent passthrough is more useful
/// to callers who can post-validate. This is a deliberate trade-off.
///
/// # Examples
///
/// ```
/// use rfc_2045_quoted_printable_decoder::decode;
///
/// let input = b"Hello=20World=\r\nThis is a test.\r\n";
/// let output = decode(input);
/// assert_eq!(output, b"Hello WorldThis is a test.\r\n");
/// ```
pub fn decode(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(input.len());
    let mut i = 0;
    let len = input.len();

    while i < len {
        let b = input[i];

        if b == b'=' {
            // Look ahead for a soft line break or hex escape.
            // We must be careful about end-of-input and line endings.
            if i + 1 >= len {
                // '=' at end of input with nothing following.
                // Not a valid soft break (no terminator). Emit verbatim.
                result.push(b);
                i += 1;
                continue;
            }

            let next = input[i + 1];

            if next == b'\r' {
                // Possible CRLF soft break.
                if i + 2 < len && input[i + 2] == b'\n' {
                    // "=\r\n" — soft break, skip all three bytes.
                    i += 3;
                    continue;
                } else {
                    // "=\r" without LF. RFC says line endings are CRLF,
                    // but bare CR is unusual. Treat as soft break since
                    // the '=' clearly intended one. Skip two bytes.
                    i += 2;
                    continue;
                }
            } else if next == b'\n' {
                // "=\n" — bare LF soft break. Skip two bytes.
                i += 2;
                continue;
            } else if is_hex_digit(next) && i + 2 < len && is_hex_digit(input[i + 2]) {
                // Valid hex escape "=XX".
                let high = hex_value(next);
                let low = hex_value(input[i + 2]);
                result.push((high << 4) | low);
                i += 3;
                continue;
            } else {
                // Malformed '=' followed by non-hex or insufficient digits.
                // Emit verbatim per our documented decision.
                result.push(b);
                i += 1;
                continue;
            }
        }

        result.push(b);
        i += 1;
    }

    result
}

/// Returns true if the byte is an ASCII hex digit (0-9, A-F, a-f).
fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// Converts a hex digit byte to its numeric value.
///
/// # Panics
///
/// This function is only called after `is_hex_digit` confirms the byte
/// is a valid hex digit, so it never panics in practice. It uses `unwrap`
/// for clarity — the precondition is established by the caller.
fn hex_value(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'A'..=b'F' => b - b'A' + 10,
        b'a'..=b'f' => b - b'a' + 10,
        _ => 0, // Unreachable given is_hex_digit precondition.
    }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn test_plain_text_passes_through() {
        assert_eq!(decode(b"Hello, World!"), b"Hello, World!");
    }

    #[test]
    fn test_hex_escape_space() {
        assert_eq!(decode(b"Hello=20World"), b"Hello World");
    }

    #[test]
    fn test_hex_escape_lowercase() {
        assert_eq!(decode(b"=3d"), b"=");
    }

    #[test]
    fn test_hex_escape_uppercase() {
        assert_eq!(decode(b"=3D"), b"=");
    }

    #[test]
    fn test_soft_break_crlf() {
        assert_eq!(decode(b"Line one=\r\nLine two"), b"Line oneLine two");
    }

    #[test]
    fn test_soft_break_lf_only() {
        assert_eq!(decode(b"Line one=\nLine two"), b"Line oneLine two");
    }

    #[test]
    fn test_multiple_soft_breaks() {
        let input = b"A=\r\nB=\r\nC";
        assert_eq!(decode(input), b"ABC");
    }

    #[test]
    fn test_high_byte_escape() {
        // =FF is byte 255
        assert_eq!(decode(b"=FF"), vec![0xFFu8]);
    }

    #[test]
    fn test_malformed_equals_passthrough() {
        // '=' followed by non-hex: emit verbatim per documented decision.
        assert_eq!(decode(b"cost=5"), b"cost=5");
    }

    #[test]
    fn test_equals_at_end_of_input() {
        // No terminator, so not a soft break. Emit verbatim.
        assert_eq!(decode(b"trailing="), b"trailing=");
    }

    #[test]
    fn test_single_hex_digit_after_equals() {
        // "=3" at end — only one hex digit, insufficient. Emit '=' verbatim
        // then continue with '3'.
        assert_eq!(decode(b"=3"), b"=3");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(decode(b""), b"");
    }

    #[test]
    fn test_preserves_whitespace() {
        assert_eq!(decode(b"  indented  "), b"  indented  ");
    }

    #[test]
    fn test_soft_break_preserves_trailing_whitespace_before_equals() {
        // Whitespace before the soft-break '=' is preserved, not stripped.
        // RFC 2045 §6.7 note (5) advises encoders strip it, but decoders
        // need not reject it. We preserve for fidelity.
        assert_eq!(decode(b"text =\r\nmore"), b"text more");
    }

    #[test]
    fn test_mixed_content() {
        let input = b"Subject:=20Test=\r\nBody=09tabbed";
        assert_eq!(decode(input), b"Subject: TestBody\ttabbed");
    }
}
