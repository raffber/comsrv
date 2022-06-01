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

## Addresses and Functionality

Addresses are kept in a VISA compatible format. If an address does not match any protocol, VISA is used as a fallback.
The address parts are separated with "::".

* Serial ports: `serial::<path-to-serial>::<baud-rate>::<settings>`, where:
    * `<path-to-serial>` may be a file path on unix (e.g. `/dev/ttyS0`) or a com-port identifier on windows (
      e.g. `COM3`)
    * The `<baud-rate>` must be given in bits/s
    * The `<settings>` string is formated according
      to [conventional notation](https://en.wikipedia.org/wiki/Serial_port#Conventional_notation)
    * Example: `serial::/dev/ttyUSB0::9600::8N1`
* Prologix instruments: `prologix::<path-to-serial>::<gpib-address>`
    * `<path-to-serial>` may be a file path on unix (e.g. `/dev/ttyS0`) or a com-port identifier on windows (
      e.g. `COM3`)
    * `<gpib-address>` is the numeric gpib address of the instrument
    * Example: `prologix::/dev/ttyUSB0::12`
* Modbus/TCP: `modbus::tcp::<socket-address>::<slave-addr>`
    * For example: `modbus::tcp::192.168.0.1:509::32`
    * The slave address is optional, if no slave address is given, 255 is used.
* Modbus/RTU: over TCP or serial ports
    * Over serial: `modbus::rtu::<baud-rate>::<serial-settings>::<slave-addr>`, for example
      `modbus::rtu::COM1::115200::8N1::32`
    * Over tcp: `modbus::rtu::<socket-address>::<slave-addr>` for example `modbus::rtu::192.168.1.20::32`
    * The slave address is optional, if no slave address is given, 255 is used.
* VXI-11 based instruments: `vxi::<ip-addr>`
    * `<ip-addr>` is the IP address of the VXI-11 instrument. Note that a port number must not be provided as the port is
      determined by the `VXI-11` port mapper protocol.
    * Example: `vxi::192.168.1.20`
* CAN bus:
    * Supports PCanBasic API on Windows (requires driver installation)
    * PCan address format follows `can::<bus-device>::<bit-rate>` e.g. `can::pcan::usb1::125000`
    * Supports any SocketCAN capable device on linux (such as PCan out of the box):
    * To address a SocketCAN device, use: `can::socket::<ifname>` e.g. `can::socket::can0`
    * Features a loopback device: `can::loopback`
* HID devices: `hid::<vendor-id>::<product-id>`
    * Example: `hid::dead::beef`
* Raw TCP stream: `tcp::<ip-addr>:<port>`
    * Exposes a raw byte stream and is thus accessible over the same API as serial ports
* Using [`libsigrok`](https://sigrok.org/wiki/Libsigrok) supported logic analyzers:
    * Use `sigrok::<sigrok-device-address>`
    * To determine their addresses, sigrok devices can be listed using the `ListSigrokDevices` subcommand
* Visa instruments: Refer to [VISA resource names](https://pyvisa.readthedocs.io/en/1.8/names.html)

## Python Interface

All classes to interact with `comsrv` are found in `comsrv`.

### Connecting to Remote Servers

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

### Connecting, Disconnecting and Listing Instruments

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

### Serial Ports and TCP streams

Serial ports and TCP streams are exposed with a byte-stream like interface. The `comsrv.ByteStreamPipe` class
provides:

* Line-framed string functions: `write_line()`, `read_line()`, `query_line()`
* COBS-framed byte functions: `cobs_write()`, `cobs_read()`, `cobs_query()`
* Low level stream access functionality, such as `write()`, `read_exact()` and more

### SCPI Instruments

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

### Modbus Instruments

Modbus/TCP and Modbus/RTU are supported using the `comsrv.modbus.ModBusDevice` class. It supports the most common ModBus functions:

* For writing: `write_registers()`, `write_coils()`
* For reading: `read_holding()`, `read_coil()`, `read_discrete()`, `read_input()`
* Custom modbus functions: `send_custom_command()`


### HID Devices

HID devices are supported using the `comsrv.hid.HidDevice` class and the `comsrv.hid.enumerate_hid_devices()` function:

* For writing: `write(data)`
* For reading: `read(timeout_ms=100)`
* Querying information: `get_info()`

### Locking Instruments

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

### libsigrok Devices

If `sigrok-cli` is installed (and reachable in `$PATH`), `comsrv` supports reading data from a logic analyzer into numpy
arrays. This functionality is provided by the `comsrv.sigrok` module.

```
from comsrv.sigrok import list_devices

devices = await list_devices()
my_device = devices[0]

t, data = await my_device.read(channels=['D1', 'D2'], samplerate=48e6, num_samples=1000)
print(data['D1'])

# or sample all channels at max sample rate
t, data = await my_device.read(num_samples=1000)
```

### Interfacing a CAN bus

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

## Examining the comsrv log

To enable logging, start `comsrv` with the `-v` option:

```
comsrv -v  # enables logging
```

The log will print executed commands as well as inform about communication errors that occur.


## RPC Interface and Wire Protocol

Refer to the [documentation of the RPC interface](rpc.md) to understand how to frame the underlying requests/responses.

The following types define the RPC requests, responses and errors.
The [serde data model](https://serde.rs/data-model.html) applies.

### Base protocol

```
enum Request {
    Scpi {
        addr: String,
        task: ScpiRequest,
    },
    ModBus {
        addr: String,
        task: ModBusRequest,
    },
    Bytes {
        addr: String,
        task: ByteStreamRequest,
    },
    Can {
        addr: String,
        task: CanRequest,
    },
    Sigrok {
        addr: String,
        task: SigrokRequest,
    },
    ListSigrokDevices,
    ListInstruments,
    DropAll,
    Shutdown,
} 

enum Response {
    Error(RpcError),
    Instruments(Vec<String>),
    Scpi(ScpiResponse),
    Bytes(ByteStreamResponse),
    ModBus(ModBusResponse),
    Can(CanResponse),
    Done,
}

enum RpcError {
    Io(String),
    Visa(VisaError),
    Disconnected,
    NotSupported,
    DecodeError(String),
    InvalidBinaryHeader,
    NotTerminated,
    InvalidAddress,
    InvalidRequest,
    Timeout,
    Vxi(String),
    Can {
        addr: String,
        err: CanError,
    },
}
```

### Byte Stream Specific Types

```
enum ByteStreamRequest {
    Write(Vec<u8>),
    ReadExact {
        count: u32,
        timeout_ms: u32,
    },
    ReadUpTo(u32),
    ReadAll,
    CobsWrite(Vec<u8>),
    CobsRead(u32), // timeout
    CobsQuery {
        data: Vec<u8>,
        timeout_ms: u32,
    },
    WriteLine {
        line: String,
        term: u8,
    },
    ReadLine {
        timeout_ms: u32,
        term: u8,
    },
    QueryLine {
        line: String,
        timeout_ms: u32,
        term: u8,
    }, 
}

enum ByteStreamResponse {
    Done,
    Data(Vec<u8>),
    String(String),
} 
```

### Modbus Specific Types

```
enum ModBusRequest {
    ReadCoil {
        addr: u16,
        cnt: u16,
    },
    ReadDiscrete {
        addr: u16,
        cnt: u16,
    },
    ReadInput {
        addr: u16,
        cnt: u16,
    },
    ReadHolding {
        addr: u16,
        cnt: u16,
    },
    WriteCoil {
        addr: u16,
        values: Vec<bool>,
    },
    WriteRegister {
        addr: u16,
        data: Vec<u16>,
    },
}

enum ModBusResponse {
    Done,
    Number(Vec<u16>),
    Bool(Vec<bool>),
}
```

### SCPI Requests/Responses

```
enum ScpiRequest {
    Write(String),
    QueryString(String),
    QueryBinary(String),
    ReadRaw,
}

enum ScpiResponse {
    Done,
    String(String),
    Binary{
        data: Vec<u8>  // IMPORTANT: serialized as base64!
    },
}
```

### CAN Requests/Responses/Errors

```
enum CanRequest {
    ListenRaw(bool),        // enable/disable broadcast of raw CAN messages
    ListenGct(bool),        // enable/disable broadcast of decoded GCT messages
    StopAll,                // stop all message broadcasts
    EnableLoopback(bool),   // loopback sent can messages to receiver
    TxRaw(Message),         // send a raw CAN message
    TxGct(GctMessage),      // send a GCT message
}

enum CanResponse {
    Started(String),        // Returns the device where decoding started
    Stopped(String),        // Returns the device where decoding stopped
    Ok,                     // Operation completed successfully 
    Raw(Message),           // A raw message was received
    Gct(GctMessage),        // A GCT message was received
} 

enum CanError {
    Io(String),
    InvalidInterfaceAddress,
    InvalidBitRate,
    PCanError(u32, String),
    BusError(BusError),
    TransmitQueueFull,
    IdTooLong,
    DataTooLong,
    InvalidMessage,
}

enum BusError {
    LightWarning,
    HeavyWarning,
    Passive,
    Off,
}
```

### Raw CAN Messages

```
enum Message {
    Data(DataFrame),
    Remote(RemoteFrame),
}

struct DataFrame {
    id: u32,
    ext_id: bool,
    data: Vec<u8>,
}

struct RemoteFrame {
    id: u32,
    ext_id: bool,
    dlc: u8,
} 
```

### GCT-CAN Messages

```
enum GctMessage {
    SysCtrl {
        src: u8,
        dst: u8,
        cmd: u16,
        value: bool,
        query: bool,
        data: Vec<u8>,
    },
    MonitoringData {
        src: u8,
        group_idx: u8,
        reading_idx: u8,
        data: Vec<u8>,
    },
    MonitoringRequest {
        src: u8,
        dst: u8,
        group_idx: u8,
        readings: u64,
    },
    Ddp {
        src: u8,
        dst: u8,
        data: Vec<u8>,
    },
    Heartbeat {
        src: u8,
        product_id: u16,
    },
}
```

### Sigrok Messages

```
struct SigrokRequest {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    channels: Vec<String>,
    acquire: Acquire,
    sample_rate: u64,
}

enum SigrokResponse {
    Data(Data),
    Devices(Vec<Device>),
}

struct Data {
    tsample: f64,
    length: usize,
    channels: HashMap<String, Vec<u8>>,
}

enum Acquire {
    Time(f32),
    Samples(u64),
}

enum SigrokError {
    UnexpectedOutput {
        code: i32,
        stdout: String,
        stderr: String,
    },
    InvalidOutput,
}

pub struct Device {
    addr: String,
    desc: String,
}
```