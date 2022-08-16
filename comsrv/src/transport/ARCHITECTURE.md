# Transport Layer Implementations

This module contains the implementations of the transport layer actor.
Such an actor manages the underlying hardware handles, such as a TCP socket or a serial port file handle.

Mostly this module dispatches to other libraries, as the low level async implementation is out of scope for this project.

## Anatomy of an IO-Actor

All IO actors use the following structure:

* Handles are only opened upon receiving a request. Thus, when they are created, the underlying handle is not yet openned.
* If an error occurs, depending on where in the stack, the IO actor evaluates the error and if the error occurred on the
transport layer, closes the connection.
* If an error occurs purely on transport layer and the IO actor can be sure that the data was never sent to the remote device,
the connection is closed, re-opened and the transaction is re-tried.
* If a request arrives after an error, the connection is automatically re-opened.

This behavior provides following benefits:

* A higher level application does not need to deal with connection management (which is more difficult than it seems)
* The application becomes immune against short disturbances, such as dropping TCP connections or serial ports which are disturbed by noise, overfull CAN buffers, ....
* Many instruments contain buggy IP stacks, the actors attempt to work around those bug by dropping/re-establishing connections
automatically.

To implement actiors, refer to the `comsrv.IoTask` helper, it already handles cancelling actors as well as it allows sending requests to itself.
Note that each actor must also implement `Instrument` as this is a requirement for the actor to be registered in an `Inventory`.
