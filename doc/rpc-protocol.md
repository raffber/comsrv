# RPC Interface and Wire Protocol

Refer to the [documentation of the RPC interface](rpc.md) to understand how to frame the underlying requests/responses.

The following types define the RPC requests, responses and errors.
The [serde data model](https://serde.rs/data-model.html) applies.

The single source of truth are the data types defined in the `protocol` crate.
Refer to `protocol/src/*.rs` for a specification of the protocol.

## Error Handling

A request is either answered with a response matching the request type or an error.
To propagate errors in the client, the answer to a request should be checked whether it matches the `Error` variant
and raise the error if it does. Otherwise, it can assume that the answer matches the specified request type.
Refer to the python protocol implemenation for an example.
