# TypeScript frontend conventions

- Model states as discriminated unions, not as combinations of booleans.
- A component receives data and callbacks; it does not fetch. Data access lives in a
  dedicated layer that can be tested and mocked.
- Loading, empty, error and success are four distinct states. Every screen implements all
  four, even if three are one line each.
- Do not disable a lint rule to silence a real problem. Fix the problem or document why
  the rule does not apply, at the narrowest possible scope.
- Interactive controls are reachable by keyboard and readable by a screen reader: use the
  native element before building a custom one.
