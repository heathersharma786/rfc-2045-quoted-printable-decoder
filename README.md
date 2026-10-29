# rfc-2045-quoted-printable-decoder

Decodes MIME quoted-printable encoded text per RFC 2045 Section 6.7, handling soft line breaks and `=XX` hex escapes.

## Usage

```rust
use rfc_2045_quoted_printable_decoder::decode;

let input = b"Hello=20World=\r\nThis is a test.\r\n";
let output = decode(input);
assert_eq!(output, b"Hello WorldThis is a test.\r\n");
```

The library exports a single function:

- `decode(input: &[u8]) -> Vec<u8>` — takes a quoted-printable encoded byte slice and returns the decoded bytes.

## Why this exists

Email bodies are frequently quoted-printable encoded to keep them within the 7-bit ASCII constraint of SMTP while preserving 8-bit data. Decoding is conceptually simple — replace `=XX` hex escapes with the corresponding byte and remove soft line breaks — but the details around line endings, malformed input, and whitespace handling are easy to get wrong.

This library exists to do exactly that one job with no dependencies and clear edge-case behavior.

## Edge cases

The decoder operates on raw bytes and returns raw bytes. It does not perform character-set transcoding — that belongs to a higher layer per RFC 2045's separation of transfer encoding from charset.

A `=` followed by something that is not a valid two-digit hex sequence or a line terminator is malformed input. Rather than error or silently drop bytes, the decoder passes the `=` through verbatim. This is a deliberate choice: real-world email often contains minor non-conformance, and silent passthrough lets callers post-validate the result rather than losing data.

Trailing whitespace (spaces or tabs) before a soft line break is preserved rather than stripped. RFC 2045 advises encoders to strip such whitespace but does not require decoders to reject it, and stripping would risk destroying legitimate whitespace elsewhere.
