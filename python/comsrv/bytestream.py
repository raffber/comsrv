from typing import Optional, Union

from . import Address, BasePipe, ComSrvError, Instrument, Rpc, duration_to_json
import re


SERIAL_ADDRESS_RE = re.compile(
    r"(?P<path>.*?)::(?P<baudrate>\d+)::(?P<config>[78][ENO][12])"
)

FTDI_ADDRESS_RE = re.compile(
    r"(?P<path>.*?)::(?P<baudrate>\d+)::(?P<config>[78][ENO][12])"
)

TCP_ADDRESS_RE = re.compile(r"(?P<host_or_ip>.*?):(?P<port>\d+)")


class ByteStreamAddress(Address):
    pass


class ByteStreamInstrument(Instrument):
    @classmethod
    def parse(cls, address_string: str):
        address_string = address_string.strip()
        if address_string.startswith("serial::"):
            address_string = address_string.removeprefix("serial::")
            m = SERIAL_ADDRESS_RE.match(address_string)
            if m is None:
                raise ValueError(
                    "Serial Address should be of the form `serial::<path>::<baudrate>::<config>`"
                )
            path, baudrate, config = cls._parse_serial_path(m)
            return SerialInstrument(
                SerialAddress(path), SerialPortConfig(config, baudrate)
            )
        elif address_string.startswith("ftdi::"):
            address_string = address_string.removeprefix("ftdi::")
            m = FTDI_ADDRESS_RE.match(address_string)
            if m is None:
                raise ValueError(
                    "FTDI Address should be of the form `serial::<path>::<baudrate>::<config>`"
                )
            path, baudrate, config = cls._parse_serial_path(m)
            return FtdiInstrument(FtdiAddress(path), SerialPortConfig(config, baudrate))
        elif address_string.startswith("tcp::"):
            address_string = address_string.removeprefix("tcp::")
            m = TCP_ADDRESS_RE.match(address_string)
            host_or_ip = m.group("host_or_ip")
            port = int(m.group("port"))
            if port < 1 or port > 65535:
                raise ValueError("Port needs to be in range (1, 65535)")
            return TcpInstrument(TcpAddress(host_or_ip, port))
        raise ValueError(
            "Invalid address. Must start with `serial::` or `ftdi::` or `tcp::`"
        )

    @classmethod
    def _parse_serial_path(cls, match):
        baudrate = int(match.group("baudrate"))
        path = match.group("path")
        config = match.group("config")
        return (path, baudrate, config)


class FtdiAddress(ByteStreamAddress):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def enum_name(self):
        return "Ftdi"


class SerialAddress(ByteStreamAddress):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def enum_name(self):
        return "Serial"


class TcpAddress(ByteStreamAddress):
    def __init__(self, host: str, port: int) -> None:
        self.port = port
        self.host = host

    def to_json(self):
        return {"port": self.port, "host": self.host}

    @property
    def enum_name(self):
        return "Tcp"


class TcpInstrument(ByteStreamInstrument):
    def __init__(self, address: TcpAddress) -> None:
        super().__init__()
        self._address = address

    @property
    def address(self) -> Address:
        return self._address

    def to_json(self):
        return {"Tcp": {"address": self.address.to_json()}}


class SerialPortConfig(object):
    def __init__(self, config: str, baudrate: int) -> None:
        self.config = config
        self.baudrate = baudrate

    def to_json(self):
        return {"config": self.config, "baudrate": self.baudrate}


class FtdiInstrument(ByteStreamInstrument):
    def __init__(self, address: FtdiAddress, port_config: SerialPortConfig) -> None:
        super().__init__()
        self._address = address
        self._port_config = port_config

    def to_json(self):
        return {
            "Ftdi": {
                "address": self.address.to_json(),
                "port_config": self._port_config.to_json(),
            }
        }

    @property
    def address(self) -> Address:
        return self._address


class SerialInstrument(ByteStreamInstrument):
    def __init__(self, address: SerialAddress, port_config: SerialPortConfig) -> None:
        super().__init__()
        self._address = address
        self._port_config = port_config

    def to_json(self):
        return {
            "Serial": {
                "address": self.address.to_json(),
                "port_config": self._port_config.to_json(),
            }
        }

    @property
    def address(self) -> Address:
        return self._address


class ByteStreamPipe(BasePipe):
    def __init__(
        self, instrument: Union[str, ByteStreamInstrument], rpc: Optional[Rpc] = None
    ):
        if not isinstance(instrument, ByteStreamInstrument):
            instrument = ByteStreamInstrument.parse(instrument)
        self._instrument = instrument
        super().__init__(instrument.address, rpc)

    @property
    def instrument(self):
        self._instrument

    async def request(self, request):
        result = await self.get(
            {
                "Bytes": {
                    "instrument": self._instrument.to_json(),
                    "request": request,
                    "lock": self._lock,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Bytes"]

    async def write(self, data: bytes):
        result = await self.request({"Write": list(data)})

    async def read_all(self) -> bytes:
        result = await self.request("ReadAll")
        data = bytes(result["Data"])
        return data

    async def read_to_term(self, term: int, timeout: float) -> bytes:
        result = await self.request(
            {"ReadToTerm": {"term": term, "timeout": duration_to_json(timeout)}}
        )
        data = bytes(result["Data"])
        return data

    async def read_exact(self, count: int, timeout: float) -> bytes:
        result = await self.request(
            {"ReadExact": {"count": count, "timeout": duration_to_json(timeout)}}
        )
        data = bytes(result["Data"])
        return data

    async def read_upto(self, count: int) -> bytes:
        result = await self.request({"ReadUpTo": count})
        data = bytes(result["Data"])
        return data

    async def cobs_write(self, data):
        await self.request({"CobsWrite": list(data)})

    async def cobs_read(self, timeout):
        result = await self.request({"CobsRead": duration_to_json(timeout)})
        data = bytes(result["Data"])
        return data

    async def cobs_query(self, data, timeout):
        result = await self.request(
            {"CobsQuery": {"data": list(data), "timeout": duration_to_json(timeout)}}
        )
        data = bytes(result["Data"])
        return data

    async def write_line(self, line: str, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        await self.request({"WriteLine": {"line": line, "term": term}})

    async def read_line(self, timeout, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.request(
            {"ReadLine": {"term": term, "timeout": duration_to_json(timeout)}}
        )
        return result["String"]

    async def query_line(self, line: str, timeout, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.request(
            {
                "QueryLine": {
                    "line": line,
                    "term": term,
                    "timeout": duration_to_json(timeout),
                }
            }
        )
        return result["String"]

    def modbus(
        self,
        station_address: int,
        protocol=None,
        timeout=1.0,
    ):
        from .modbus import ModBusProtocol, ModBusDevice

        if protocol is None:
            protocol = ModBusProtocol.RTU
        return ModBusDevice(
            self, protocol=protocol, station_address=station_address, timeout=timeout
        )
