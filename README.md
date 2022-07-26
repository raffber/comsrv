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

## Examining the comsrv log

To enable logging, start `comsrv` with the `-v` option:

```sh
comsrv -v  # enables logging
```

The log will print executed commands as well as inform about communication errors that occur.
