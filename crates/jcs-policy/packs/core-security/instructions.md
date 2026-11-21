# Security baseline

- Treat every value that crosses a process boundary as hostile until validated: HTTP
  bodies, query strings, headers, files, environment variables, CLI arguments.
- Prefer parsing into a type that cannot hold an invalid value over validating later.
- Authentication decides *who*; authorisation decides *what*. Both are required at every
  entry point, including "internal" ones.
- Errors shown to users explain what to do, not how the system is built.
- If you find a secret in the repository, stop and report it. Do not "clean it up" in a
  commit that also changes behaviour: rotation comes first.
