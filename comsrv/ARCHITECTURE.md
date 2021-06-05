# Architecture of comsrv

## Lifecycle

The application is started in the `main()` function in `main.rs`, where an `App` object is created. The `App`
is `Send + Clone` and contains some shared data (such as a registry of all instruments).

The `App` contains the `App::run()` actor which is the main actor of the application.

## Actors and Interaction between them

Most instruments are spawned as an actor. When a request is processed, the request is forwarded to that actor. The actor
takes care of managing the instrument state (such as establishing a connection) and processes the request.

The request are checked and dispatched from the main actor in `App::run()`.

## Addresses, Request and Response

The `Request` and `Response` types are defined in `app.rs` and define the wire format. The main handler in the application
matches a request and parses the address. An additional step is then required to verify whether the given address
is valid for the request at hand.

An address has a `address.handle_id()` function which returns a handle that maps directly to a hardware resource and
is used to identify the instrument. Once the instrument has been found, the request is passed to the instrument.

The `Inventory` keeps track of all instruments and maps `HandleId -> Instrument`.
It also provides the locking feature. Thus access to an instrument might be delayed until a lock has been released.

## Code Map

* `main.rs` - Entry point. Spawns the `App::run()` actor
* `app.rs` - Defines the root RPC application as `App` and provides the functionality to dispatch to the relevant
  submodules and collects application data.
* `lib.rs` - Defines some globally used types, such as the `comsrv::Error` type.

### Infrastructure

* `instrument.rs` - Defines the `Address` and the `Instrument`.
  The `Instrument`
  object is an enum over all supported instrument types.
* `address.rs` -  Each `Address` points to an `Instrument`.
* `inventory.rs` - The `Inventory` keeps track of all connected instruments and is sharable between threads.

### Instruments and Interfaces

* `bytestream.rs` - Defines operations on a bytes stream (i.e. `AsyncRead + AsyncWrite`). This is applicable to all
  bytestream-like interfaces such as tcp streams or serial ports.
* `tcp.rs` - Manages TCP streams and exposes `tcp::Instrument`.
* `modbus.rs` - Makes use of `tokio-modbus` to send/receive modbus requests. Exposes `modbus::Instrument`.
* `vxi.rs` - Makes use of `async-vxi11` and provides an SCPI interface for VXI enabled instruments.
  Exposes `vxi::Instrument`.
* `can/` - Dispatches requests to `async-can` and decodes can messages. Exposes `can::Instrument`.
* `visa/` - Dispatches requests to the NI-VISA library. Exposes `visa::Instrument`.
* `serial/` - Manages serial ports and exposes `serial::Instrument`.
* `sigrok.rs` - Interfaces the `sigrok-cli` command line application. It operates stateless.

### Auxiliary Code

* `cobs.rs` - COBS encoder/decoder
* `iotask.rs` - A helper struct to build an actor for managing an instrument
* `scpi.rs` - Some SCPI interface code and type definitions
* `util.rs` - See name...
