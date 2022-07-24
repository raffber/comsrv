from audioop import add
from typing import Optional, Union

from python.comsrv.modbus import ModBusDevice, ModBusProtocol

from . import Rpc, duration_to_json, get, ComSrvError, BasePipe
from . import Client


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
        return result

    async def write(self, data: bytes):
        result = await self.request({"Write": list(data)})
        ComSrvError.check_raise(result)

    async def read_all(self) -> bytes:
        result = await self.get("ReadAll")
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def read_to_term(self, term: int, timeout: float) -> bytes:
        result = await self.get(
            {"ReadToTerm": {"term": term, "timeout": duration_to_json(timeout)}}
        )
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def read_exact(self, count: int, timeout: float) -> bytes:
        result = await self.get(
            {"ReadExact": {"count": count, "timeout": duration_to_json(timeout)}}
        )
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def read_upto(self, count: int) -> bytes:
        result = await self.get({"ReadUpTo": count})
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def cobs_write(self, data):
        result = await self.get({"CobsWrite": list(data)})
        ComSrvError.check_raise(result)

    async def cobs_read(self, timeout):
        result = await self.get({"CobsRead": duration_to_json(timeout)})
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def cobs_query(self, data, timeout):
        result = await self.get(
            {"CobsQuery": {"data": list(data), "timeout": duration_to_json(timeout)}}
        )
        ComSrvError.check_raise(result)
        data = bytes(result["Bytes"]["Data"])
        return data

    async def write_line(self, line: str, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.get({"WriteLine": {"line": line, "term": term}})
        ComSrvError.check_raise(result)

    async def read_line(self, timeout, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.get(
            {"ReadLine": {"term": term, "timeout": duration_to_json(timeout)}}
        )
        ComSrvError.check_raise(result)
        return result["Bytes"]["String"]

    async def query_line(self, line: str, timeout, term: Union[int, str] = "\n"):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.get(
            {
                "QueryLine": {
                    "line": line,
                    "term": term,
                    "timeout": duration_to_json(timeout),
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Bytes"]["String"]

    def modbus(self, station_address: int, protocol: ModBusProtocol) -> ModBusDevice:
        raise NotImplementedError
