# `comsrv` Communication Relay

The `comsrv` utility exposes an RPC interface for communicating with instruments. Currently, the following type of
instruments are supported:

* Serial ports
* VXI-11/LXI Instruments
* Prologix USB-to-GPIB adapters
* Modbus/TCP instruments
* Modbus/RTU over serial ports or TCP streams
* Raw tcp byte streams
* CAN messaging
* All VISA controlled instruments as fallback

`comsrv` automatically manages connections and operating system handles:

* If no connection has been established, the connection is automatically opened
* If the connection drops and a new request is issued, the connection is automatically reopened
* In case the configuration of a instrument changes (such as the baud-rate on a serial port), it is appropriately re-initialized

Thus, the application using the `comsrv` should not care about how connections are managed to instruments. It must also not care about spawning threadpools for IO, bridging async and sync interface or worry about library support for exotic protcol. Also, error handling can be offloaded to large part.

The `comsrv` RPC protocol uses standard technology, specifically HTTP, WebSockets and JSON to simplify integration with various stacks or programming languages. It is even possible to directly interact with the `comsrv` using the web browser.

Furthermore, it allows connecting from different processes to the same OS handles in a synchronized way. Clients may lock instruments to fine grained control, otherwise RPC requests are just queued and processed in the order that they are arriving, which is sufficient for most applications.

## Examining the comsrv log

To enable logging, start `comsrv` with the `-v` option:

```sh
comsrv -v  # enables logging
```

The log will print executed commands as well as inform about communication errors that occur.
