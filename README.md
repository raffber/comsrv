# `comsrv` Communication Relay

[![Rust Stable](https://github.com/raffber/comsrv/actions/workflows/ci.yml/badge.svg)](https://github.com/raffber/comsrv/actions/workflows/ci.yml)

The `comsrv` utility exposes an [RPC interface](https://github.com/raffber/wsrpc) mainly intended for communicating with lab instruments.
Currently, the following type of instruments are supported:

* Serial Ports
* VXI-11/LXI Instruments
* Prologix USB-to-GPIB adapters
* Modbus/TCP instruments
* Modbus/RTU over serial ports or TCP streams
* Raw TCP Sockets
* USB HID based devices
* FTDI devices
* CAN over PCAN and SocketCAN (linux-only)
* All VISA controlled instruments as fallback (requires VISA installation)

`comsrv` automatically manages connections and operating system handles:

* If no connection has been established, the connection is automatically opened
* If the connection drops and a new request is issued, the connection is automatically reopened
* In case the configuration of a instrument changes (such as the baud-rate on a serial port), it is appropriately re-initialized

Thus, the application using the `comsrv` should not care much about how connections are managed. It must also not care about spawning threadpools for IO, bridging async and sync interface or worry about library support for exotic protcols. As a consequence most of the connectivity error handling is offloaded
to the `comsrv`.

The RPC protocol uses standard technology, specifically HTTP, WebSockets and JSON to simplify integration with various stacks or programming languages. The reference implementation is found [here](https://github.com/raffber/wsrpc). It is even possible to directly interact with the `comsrv` using the web browser.

Furthermore, it allows connecting from different processes to the same OS handles in a synchronized way. Clients may lock instruments to fine grained control, otherwise RPC requests are just queued and processed in the order that they are arriving, which is sufficient for most applications.

## Examining the comsrv log

To enable logging, start `comsrv` with the `-v` option:

```sh
comsrv -v  # enables logging
```

The log will print executed commands as well as inform about communication errors that occur.

## Documentation

 * The RPC protocol is: https://github.com/raffber/wsrpc. `comsrv` specific information [here](doc/rpc-protocol.md).
 * The Python API is described [here](doc/python_api.md).
 * Optional VISA-compatible resource strings for short-hand initialization of instrument are documented [here](doc/python_resource_strings.md).

## License

Licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
