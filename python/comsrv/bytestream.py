from typing import Optional, Union

from python.comsrv.modbus import ModBusDevice, ModBusProtocol

from . import Address, BasePipe, ComSrvError, Instrument, Rpc, duration_to_json
import re


SERIAL_ADDRESS_RE = re.compile(
    r"serial::(?P<path>.*?)::(?P<baudrate>\d+)::(?P<config>[78][ENO][12])"
)

FTDI_ADDRESS_RE = re.compile(r"TODO")


class FtdiAddress(Address):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def enum_name(self):
        return "Ftdi"


class SerialAddress(Address):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def enum_name(self):
        return "Serial"


class TcpAddress(Address):
    def __init__(self, host: str, port: int) -> None:
        self.port = port
        self.host = host

    def to_json(self):
        return {"port": self.port, "host": self.host}

    @property
    def enum_name(self):
        return "Tcp"


class ByteStreamInstrument(Instrument):
    @classmethod
    def parse(cls, addr_string):
        raise NotImplementedError


class TcpInstrument(ByteStreamInstrument):
    def __init__(self, address: TcpAddress) -> None:
        super().__init__()
        self._address = address

    def address(self) -> Address:
        return self._address

    def to_json(self):
        return {"address": self.address.to_json()}

    @property
    def enum_name(self):
        return "Tcp"


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
        return {"address": self.address.to_json()}

    @property
    def enum_name(self):
        return "Ftdi"

    def address(self) -> Address:
        return self._address


class SerialPortInstrument(ByteStreamInstrument):
    def __init__(self, address: SerialAddress, port_config: SerialPortConfig) -> None:
        super().__init__()
        self._address = address
        self._port_config = port_config

    def to_json(self):
        return {"address": self.address.to_json()}

    @property
    def enum_name(self):
        return "Serial"

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
        protocol: ModBusProtocol = ModBusProtocol.RTU,
        timeout=1.0,
    ) -> ModBusDevice:
        return ModBusDevice(
            self, protocol=protocol, station_address=station_address, timeout=timeout
        )
