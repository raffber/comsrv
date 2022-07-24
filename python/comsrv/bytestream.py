from typing import Optional, Union

from python.comsrv.modbus import ModBusDevice, ModBusProtocol

from . import Address, Rpc, duration_to_json, ComSrvError, BasePipe


class FtdiAddress(Address):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def type_name(self):
        return "Ftdi"


class SerialAddress(Address):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self):
        return {"port": self.port}

    @property
    def type_name(self):
        return "Serial"


class TcpAddress(Address):
    def __init__(self, host: str, port: int) -> None:
        self.port = port
        self.host = host

    def to_json(self):
        return {"port": self.port, "host": self.host}

    @property
    def type_name(self):
        return "Tcp"


class ByteStreamInstrument(object):
    def instrument(self):
        raise NotImplementedError

    @classmethod
    def parse(cls, addr_str):
        raise NotImplementedError

    @property
    def address(self):
        raise NotImplementedError

    def to_json(self):
        raise NotImplementedError


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
