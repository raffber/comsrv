# Resource Strings in Python API

The python API supports a string format to encode instrument and device configuration. This also allows the `ScpiPipe` to fall back on Visa implementations if required. The string identifiers are useful for specifying resources without many imports and complicated constructor invocations, but are entirely optional and just represent "syntactic sugar".

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
* Modbus over TCP Socket: `modbus::<protocol>::<socket-address>::<slave-addr>`
  * Example: `modbus::tcp::192.168.0.1:509::32`
  * The slave address is optional, if no slave address is given, 255 is used.
* Modbus over Serial Ports:
  * Over serial: `modbus::<protocol>::<baud-rate>::<serial-settings>::<slave-addr>`, for example
      `modbus::rtu::COM1::115200::8N1::32`
  * The slave address is optional, if no slave address is given, 255 is used.
* VXI-11 based instruments: `vxi::<ip-addr>`
  * `<ip-addr>` is the IP address of the VXI-11 instrument. Note that a port number must not be provided as the port is determined by the `VXI-11` port mapper protocol.
  * Example: `vxi::192.168.1.20`
* CAN bus:
  * Supports PCanBasic API on Windows (requires driver installation)
  * PCan address format follows `can::<bus-device>::<bit-rate>` e.g. `can::pcan::usb1::125000`
  * Supports any SocketCAN capable device on linux (such as PCan out of the box):
  * To address a SocketCAN device, use: `can::socket::<ifname>` e.g. `can::socket::can0`
  * Features a loopback device: `can::loopback`
* Raw TCP stream: `tcp::<ip-addr>:<port>`
  * Exposes a raw byte stream and is thus accessible over the same API as serial ports
* Using [`libsigrok`](https://sigrok.org/wiki/Libsigrok) supported logic analyzers:
  * Use `sigrok::<sigrok-device-address>`
  * To determine their addresses, sigrok devices can be listed using the `ListSigrokDevices` subcommand
* Visa instruments: Refer to [VISA resource names](https://pyvisa.readthedocs.io/en/1.8/names.html)
