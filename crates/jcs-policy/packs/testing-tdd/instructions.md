# Testing conventions

- A test name states the behaviour: `rejects_expired_token`, not `test_auth_2`.
- Arrange, act, assert — in that order, visibly.
- Prefer a real object to a mock. Mock the boundary you do not own, not the code under test.
- If a test needs a sleep to pass, the code has an observable readiness signal missing.
- When a test is flaky, treat it as a defect in the test or in the code, never as noise.
- For a bug fix, the reproducing test must fail on the previous commit and pass on the fix.
