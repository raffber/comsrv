import base64
from typing import Optional, Union

from . import ComSrvError, BasePipe, Rpc
from .bytestream import ByteStreamPipe


class ScpiAddress(object):
    pass


class ScpiInstrument(object):
    def __init__(self, address: ScpiAddress) -> None:
        self._address = address

    def address(self):
        return self._address

    def parse(self, instrument: str):
        raise NotImplementedError


class ScpiPipe(BasePipe):
    def __init__(
        self, instrument: Union[str, ScpiInstrument], rpc: Optional[Rpc] = None
    ):
        if isinstance(instrument, str):
            instrument = ScpiInstrument.parse(instrument)
        super().__init__(instrument.address, rpc)

    async def request(self, request):
        result = await self.get(
            {
                "Scpi": {
                    "instrument": self._instrument,
                    "request": request,
                    "lock": self._lock,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Scpi"]

    async def query(self, msg: str) -> str:
        result = await self.get({"QueryString": msg})
        return result["String"]

    async def write(self, msg: str):
        await self.get({"Write": msg})

    async def query_binary(self, msg: str) -> bytes:
        result = await self.get({"QueryBinary": msg})
        data = result["Binary"]["data"]
        return base64.b64decode(data)

    async def read_raw(self) -> bytes:
        result = await self.get("ReadRaw")
        ComSrvError.check_raise(result)
        data = result["Binary"]["data"]
        return base64.b64decode(data)


class SerialScpiPipe(BasePipe):
    def __init__(self, bs_pipe: ByteStreamPipe, term="\n", timeout=1.0):
        self._inner = bs_pipe
        self._timeout = timeout
        self._term = term

    @property
    def term(self):
        return self._term

    @term.setter
    def term(self, value):
        self._term = value

    @property
    def timeout(self):
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        self._timeout = value

    async def query(self, msg: str) -> str:
        return await self._inner.query_line(msg, self._timeout, term=self._term)

    async def write(self, msg: str):
        return await self._inner.write_line(msg, term=self._term)

    async def query_binary(self, msg: str) -> bytes:
        raise NotImplementedError

    async def read_raw(self) -> bytes:
        return await self._inner.read_line(self._timeout, term=self._term)
