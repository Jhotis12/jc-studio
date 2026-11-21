//! Incremental Server-Sent Events decoder.
//!
//! Deliberately hand-written: the OpenCode v2 stream does **not** use the `event:`
//! field for its event names. It sends everything as `data:` lines whose JSON body
//! carries a `type` field, plus `: heartbeat` comments. A generic SSE crate would
//! mis-dispatch on that, so we own the framing and the tests.
//!
//! Implements the parts of the SSE spec that matter here: line framing over CR/LF/CRLF,
//! comment lines, multi-line `data` joined with newlines, and dispatch on blank lines.

/// One dispatched SSE event.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseFrame {
    /// Value of the `event:` field, if the producer used one.
    pub event: Option<String>,
    /// Value of the `id:` field, if any.
    pub id: Option<String>,
    /// Concatenated `data:` payload.
    pub data: String,
    /// Reconnection time advertised by `retry:`.
    pub retry: Option<u64>,
}

impl SseFrame {
    /// Whether the frame carries no usable payload.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty() && self.event.is_none()
    }
}

/// Feed bytes in, get complete frames out. Holds partial lines across chunks.
#[derive(Debug, Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    data: String,
    event: Option<String>,
    id: Option<String>,
    retry: Option<u64>,
    has_data: bool,
    in_frame: bool,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes a chunk and returns every frame it completed.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseFrame> {
        self.buffer.extend_from_slice(chunk);

        let mut frames = Vec::new();
        while let Some((end, sep_len)) = find_line_end(&self.buffer) {
            let line: Vec<u8> = self.buffer[..end].to_vec();
            self.buffer.drain(..end + sep_len);

            if let Some(frame) = self.process_line(&line) {
                frames.push(frame);
            }
        }
        frames
    }

    /// Flushes any pending frame at end of stream.
    pub fn finish(&mut self) -> Option<SseFrame> {
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            if let Some(frame) = self.process_line(&line) {
                return Some(frame);
            }
        }
        self.dispatch()
    }

    fn process_line(&mut self, raw: &[u8]) -> Option<SseFrame> {
        let line = String::from_utf8_lossy(raw);

        if line.is_empty() {
            return self.dispatch();
        }
        // A line starting with a colon is a comment (used for keep-alives).
        if line.starts_with(':') {
            return None;
        }

        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            // A field with no colon has an empty value.
            None => (line.as_ref(), ""),
        };

        match field {
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
                self.in_frame = true;
            }
            "event" => {
                self.event = Some(value.to_string());
                self.in_frame = true;
            }
            "id" => {
                self.id = Some(value.to_string());
                self.in_frame = true;
            }
            "retry" => {
                self.retry = value.parse().ok();
                self.in_frame = true;
            }
            // Unknown fields must be ignored per spec.
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseFrame> {
        if !self.in_frame && !self.has_data {
            return None;
        }
        let frame = SseFrame {
            event: self.event.take(),
            id: self.id.take(),
            data: std::mem::take(&mut self.data),
            retry: self.retry.take(),
        };
        self.has_data = false;
        self.in_frame = false;
        Some(frame)
    }
}

/// Finds the terminator of the first line, returning (index, separator length).
///
/// A trailing lone `\r` is treated as incomplete: it may be the first half of a CRLF
/// that arrives in the next chunk.
fn find_line_end(buffer: &[u8]) -> Option<(usize, usize)> {
    for (index, byte) in buffer.iter().enumerate() {
        match byte {
            b'\n' => return Some((index, 1)),
            b'\r' => {
                if index + 1 < buffer.len() {
                    let sep = if buffer[index + 1] == b'\n' { 2 } else { 1 };
                    return Some((index, sep));
                }
                return None;
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(input: &str) -> Vec<SseFrame> {
        let mut decoder = SseDecoder::new();
        let mut frames = decoder.feed(input.as_bytes());
        if let Some(tail) = decoder.finish() {
            frames.push(tail);
        }
        frames
    }

    #[test]
    fn decodes_a_minimal_data_only_event() {
        let frames = decode_all("data: {\"type\":\"session.created\"}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "{\"type\":\"session.created\"}");
        assert!(frames[0].event.is_none());
    }

    #[test]
    fn joins_multiline_data_with_newlines() {
        let frames = decode_all("data: one\ndata: two\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "one\ntwo");
    }

    #[test]
    fn ignores_comment_lines_including_heartbeats() {
        let frames = decode_all(": heartbeat\n\n: another\n");
        assert!(frames.is_empty(), "comments must not dispatch: {frames:?}");
    }

    #[test]
    fn accepts_crlf_and_bare_cr_line_endings() {
        assert_eq!(decode_all("data: a\r\n\r\n").len(), 1);
        assert_eq!(decode_all("data: b\r\r").len(), 1);
    }

    #[test]
    fn data_without_space_after_colon_is_valid() {
        let frames = decode_all("data:{\"x\":1}\n\n");
        assert_eq!(frames[0].data, "{\"x\":1}");
    }

    #[test]
    fn splits_multiple_events_in_one_chunk() {
        let frames = decode_all("data: 1\n\ndata: 2\n\n");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "1");
        assert_eq!(frames[1].data, "2");
    }

    #[test]
    fn survives_arbitrary_chunk_boundaries() {
        let input = "data: {\"type\":\"session.text.delta\"}\n\ndata: second\n\n";
        let mut decoder = SseDecoder::new();
        let mut frames = Vec::new();
        // One byte at a time: the worst case for a streaming parser.
        for byte in input.as_bytes() {
            frames.extend(decoder.feed(&[*byte]));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "{\"type\":\"session.text.delta\"}");
        assert_eq!(frames[1].data, "second");
    }

    #[test]
    fn a_trailing_lone_cr_waits_for_the_next_chunk() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.feed(b"data: x\r").is_empty());
        let frames = decoder.feed(b"\n\r\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "x");
    }

    #[test]
    fn finish_flushes_an_unterminated_frame() {
        let mut decoder = SseDecoder::new();
        assert!(decoder.feed(b"data: tail\n").is_empty());
        let frame = decoder.finish().expect("pending frame");
        assert_eq!(frame.data, "tail");
    }

    #[test]
    fn captures_event_and_id_fields_when_present() {
        let frames = decode_all("event: tick\nid: 42\ndata: x\n\n");
        assert_eq!(frames[0].event.as_deref(), Some("tick"));
        assert_eq!(frames[0].id.as_deref(), Some("42"));
    }

    #[test]
    fn unknown_fields_are_ignored_not_errors() {
        let frames = decode_all("foo: bar\ndata: x\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "x");
    }

    #[test]
    fn blank_lines_between_comment_only_blocks_do_not_dispatch() {
        let frames = decode_all(": hi\n\n: there\n\ndata: real\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "real");
    }
}
