# Architecture of comsrv

## Lifecycle

The application is started in the `main()` function in `main.rs`, where an `App` object is created. The `App`
is `Send + Clone` and contains some shared data (such as a registry of all instruments).

The `App` contains the `App::run()` actor which is the main actor of the application.

## Protocol Structure

The protocol crate acts as a single source of truth for the RPC protocol. On a top-level, `Request` objects
are sent to the `comsrv`. If the request is well-formed it answers with a `Response` object.
The `Request` and `Response` types are defined in the protocol crate and define the wire format.

Errors are propageted with the `Response::Error` variant, hence the impl `Into<Response> for crate::Result<Response>>`.

The protocol has 3 main concepts:

* Addresses defines the hardware endpoint used by an instrument. Thus, each address maps to an actor managing the hardware.
* An instrument is the combination of an address, which identifies the hardware interface, and it's configuration.
  For example for a serial port this includes baudrate and flow control. For a tcp instrument it contains the connection timeout and the max. idle time for the connection.
* A request to an instrument. E.g. `ByteStreamRequest`. It contains all information to complete a single transaction. E.g. a modbus request with a given protocol (TCP, RTU), station address and more.

Requests are not directly associated with instrument: For example a TCP stream behaves very similar to serial port
streams - both are full-duplex byte streams. However, CAN on the other side is message oriented thus does not map
to the bytestream pattern.

## Error Type

The `crate::Error` type splits the errors into 4 categories:

* Transport related errors: Captures errors that occurs on the transport layer, e.g. when trying (and failing) to establish a TCP connection. Or a serial port failing.
* Protocol related errors: Captures errors that occur when processing on a protocol level, i.e. the transport layer is ok but the remote device gives an invalid answer or a timeout occurs.
* Argument errors: While most errors are already caught in the type system of the RPC layer, there are still many ways to give invalid input to the `comsrv`. Such errors are captured in this category.
* Internal errors: Internal errors in the `comsrv`. Those should not usually occur. Seeing such an error means there is a bug. Use `RUST_BACKTRACE=1` get the backtrace in the error.

## Actors and Interaction between them

Most instruments are spawned as actors. When a request is processed, the request is forwarded to that actor. The actor takes care of managing the instrument state (such as establishing a connection) and processes the request.
The `IoTask` type (and friends) facilitates creating such actors.

Each io actor manages its associated hardware handle, for example it opens and closes the tcp connection. Most actors simply hold the handle as `Option<Handle>` and dynamically open or close the respective handle.
For example, if a USB-to-serial is removed, the actor will stay around but drop it's handle. When the device is plugged back in, the same actor will resume the work.

Instrument actors are collected in `Inventory<Instrument>` objects.

The request are checked and dispatched from the main actor in `App::run()`.

## Code Map

* `main.rs` - Entry point. Spawns the `App::run()` actor
* `app.rs` - Defines the root RPC application as `App` and provides the functionality to dispatch to the relevant
  submodules and collects application data.
* `lib.rs` - Defines some globally used types, such as the `comsrv::Error` type.
* `iotask.rs` - Defines a lightweight actor for IO operations
* `inventory.rs` - The `Inventory` keeps track of all connected instruments and is sharable between threads.
* `protocol/*` - Defines protocol related functionality, for example the modbus protocol or generic functions based on `AsyncWrite` and `AsyncRead`.
* `transport/*` - Contains the implementation of the IO actors
