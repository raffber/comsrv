# Protocol Implementations

Protocol implementations are kept independent of the underlying "hardware" (operating system handle) implementation.
Hence, these implementations should use abstraction interfaces such as `AsyncRead` or `AsyncWrite`.
