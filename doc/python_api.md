# Python Interface

All classes to interact with `comsrv` are found in `comsrv`.

## Connecting to Remote Servers

All classes or functions take an optional `url=None` parameter, which is used determine the server url. By default, all
classes will attempt to connect to the RPC interface on `127.0.0.1` and the default ports. The default url can be
customized by calling `setup_default()` and is useful for debugging a test system remotely:

```python
from comsrv import setup_default

setup_default(host='192.168.1.12')  # can also customize http_port=... and ws_port=...

from comsrv import ScpiPipe

pipe = ScpiPipe('vxi::192.168.1.20')
# pipe will now connect to the comsrv running on 192.168.1.12
```

## Connecting, Disconnecting and Listing Instruments

All instruments are connected when they are first accessed. To disconnect instruments, the `comsrv.ComSrv` class
may be used:

```python
from comsrv import ComSrv

comsrv = ComSrv()

# drop a specific instrument
await comsrv.drop('vxi::192.168.1.20')

# drop all instrument
await comsrv.drop_all()

# list all currently connected instruments
await comsrv.list_instruments()

# shutdown the communication server
await comsrv.shutdown()
```

## Serial Ports and TCP streams

Serial ports and TCP streams are exposed with a byte-stream like interface. The `comsrv.ByteStreamPipe` class
provides:

* Line-framed string functions: `write_line()`, `read_line()`, `query_line()`
* COBS-framed byte functions: `cobs_write()`, `cobs_read()`, `cobs_query()`
* Low level stream access functionality, such as `write()`, `read_exact()` and more

## SCPI Instruments

While the `scpi` module provides abstract interfaces to interact with an SCPI interface, the `comsrv.scpi`
provides concrete implementations to use those interfaces with `comsrv`.

Specifically, the `comsrv.scpi.ScpiPipe` uses allows communicating over `vxi::*`, VISA or
the [prologix usb-to-gpib adapter](http://prologix.biz/).

```python
from comsrv import ScpiPipe
from scpi import Instrument

vxi_instr = Instrument(ScpiPipe('vxi::192.168.1.12'))
visa_instr = Instrument(ScpiPipe('tcpip::192.168.1.12::instr'))
prologix_instr = Instrument('prologix::COM3::10')  # note that, if ScpiPipe() is skipped, it is treated as default

print(await vxi_instr.identifier())  # prints the result of *IDN?
```

If an instruments uses line-delimited SCPI commands in a byte-stream, the `SerialScpiPipe` may be used. A custom line
termination may be defined with the `term` argument (default: `'\n'`):

```python
from comsrv import SerialScpiPipe
from scpi import Instrument

tcp_instr = Instrument(SerialScpiPipe('tcp::192.168.1.12:123'))
# ... or ...
serial_instr = Instrument(SerialScpiPipe('serial::COM1::9600::8N1', term='\r'))
```

## Modbus Instruments

Modbus/TCP and Modbus/RTU are supported using the `comsrv.modbus.ModBusDevice` class. It supports the most common ModBus functions:

* For writing: `write_registers()`, `write_coils()`
* For reading: `read_holding()`, `read_coil()`, `read_discrete()`, `read_input()`
* Custom modbus functions: `send_custom_command()`

## HID Devices

HID devices are supported using the `comsrv.hid.HidDevice` class and the `comsrv.hid.enumerate_hid_devices()` function:

* For writing: `write(data)`
* For reading: `read(timeout_ms=100)`
* Querying information: `get_info()`

## Locking Instruments

Modbus, SCPI, serial port and tcp based instruments support a locking feature. If a pipe instance acquires a lock an
instrument, another instance must wait until the lock is released before accessing the instrument. A lock always has to
be specified with a timeout, to ensure that a crashing process (which does not actively release a lock)
can hang-up other processes. The lock can be acquired using two different APIs:

* Calling `await pipe.lock(timeout=2.0)` and subsequently calling `await pipe.unlock()`
* Using an async context manager: `async with pipe: ....`. This is the recommended approach as it ensures proper release
  of the lock even if a command within the `async with` block fails.

The following example is given with a tcp instrument:

```python
from comsrv import ByteStreamPipe

pipe = ByteStreamPipe('tcp:1.2.3.4:1234')

print(pipe.lock_time)  # prints the default lock time, which is 1s
pipe.lock_time = 10.0  # sets the lock time to 10s

await pipe.lock()  # locks the pipe for 10s, as set above
await pipe.unlock()  # release acquired lock

async with pipe:  # the pipe is locked here
    print(pipe.locked)  # prints True
    # the pipe is unlocked here
print(pipe.locked)  # prints False
```

## libsigrok Devices

If `sigrok-cli` is installed (and reachable in `$PATH`), `comsrv` supports reading data from a logic analyzer into numpy
arrays. This functionality is provided by the `comsrv.sigrok` module.

```python
from comsrv.sigrok import list_devices

devices = await list_devices()
my_device = devices[0]

t, data = await my_device.read(channels=['D1', 'D2'], samplerate=48e6, num_samples=1000)
print(data['D1'])

# or sample all channels at max sample rate
t, data = await my_device.read(num_samples=1000)
```

## Interfacing a CAN bus

The `comsrv` service supports interfacing a CAN bus, interfaced over `comsrv.can.CanBus`, and provides
encoding/decoding facilities for:

* Raw CAN messages - using the `CanBus.raw()` receiver
* The GCT charger CAN protocol - using the `CanBus.gct()` receiver

```python
from comsrv import CanBus
from comsrv.can import DataMessage

# connect to the can bus
bus = await CanBus('can::pcan::usb1::125000').connect()

# receive 10 messages
with bus.raw() as raw_messages:
    for k in range(10):
        msg = await raw_messages.next()
        print(msg)

# send a message
msg = DataMessage()
msg.canid = 0x123
msg.data = [1, 2, 3]
await bus.send(msg)
```
