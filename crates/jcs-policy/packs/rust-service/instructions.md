# Rust service conventions

- Error types carry context: what failed, with which input, and what the caller can do.
- `unwrap()` in a test is a test failure with a message. In production code it is a panic
  with no context.
- Public APIs take `&str`/`&[T]` where ownership is not needed; return owned values.
- Async functions must not block: use `tokio::task::spawn_blocking` for CPU or blocking IO.
- Every `spawn` has an owner that will observe its failure. Fire-and-forget tasks are a
  source of silent corruption.
- Prefer `#[derive(Debug)]` and explicit trait bounds over clever generics.
