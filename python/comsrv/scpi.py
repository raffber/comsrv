import base64
from typing import Optional, Union

from . import Address, ComSrvError, BasePipe, Instrument, Rpc, duration_to_json
from .bytestream import ByteStreamInstrument, ByteStreamPipe, SerialAddress
from broadcast_wsrpc import JsonType, JsonObject


class ScpiAddress(Address):
    pass


class VxiAddress(ScpiAddress):
    def __init__(self, host: str) -> None:
        self.host = host
        super().__init__()

    def to_json(self) -> JsonType:
        return self.host

    @property
    def enum_name(self) -> str:
        return "Vxi"


class VisaAddress(ScpiAddress):
    def __init__(self, visa_address_string: str):
        super().__init__()
        self.visa_address_string = visa_address_string

    def to_json(self) -> JsonType:
        return self.visa_address_string

    @property
    def enum_name(self) -> str:
        return "Visa"


class ScpiInstrument(Instrument):
    def __init__(self, address: ScpiAddress | SerialAddress):
        self._address = address

    @property
    def address(self) -> ScpiAddress | SerialAddress:
        return self._address

    @classmethod
    def parse(cls, instrument: str) -> "ScpiInstrument":
        if instrument.startswith("vxi::"):
            splits = instrument.split("::")
            if len(splits) != 2:
                raise ValueError(
                    "Vxi address expected in the form `vxi::<host-name-or-ip>`"
                )
            return VxiInstrument(VxiAddress(splits[1]))
        if instrument.startswith("visa::"):
            instrument = instrument.replace("visa::", "", 1)
            return VisaInstrument(VisaAddress(instrument))
        if instrument.startswith("prologix::"):
            splits = instrument.split("::")
            if len(splits) != 3:
                raise ValueError(
                    "Prologix address expected in the form `prologix::<serial-port>::<gpib-address>`"
                )
            serial_port = splits[1]
            gpib_address = int(splits[2])
            return PrologixInstrument(
                SerialAddress(serial_port), gpib_address=gpib_address
            )
        else:
            return VisaInstrument(VisaAddress(instrument))


class VxiInstrument(ScpiInstrument):
    def to_json(self) -> JsonObject:
        return {"Vxi": {"host": self._address.to_json()}}


class VisaInstrument(ScpiInstrument):
    def to_json(self) -> JsonObject:
        return {"Visa": {"address": self._address.to_json()}}


class PrologixInstrument(ScpiInstrument):
    def __init__(self, address: SerialAddress, gpib_address: int) -> None:
        self._gpib_address = gpib_address
        super().__init__(address)

    @property
    def gpib_address(self) -> int:
        return self._gpib_address

    def to_json(self) -> JsonObject:
        return {"address": self._address.to_json()}


class Transport(object):
    async def request(
        self,
        request: JsonType,
        timeout: float | None = None,
    ) -> JsonType:
        raise NotImplementedError


class ScpiTransport(Transport):
    def __init__(self, instrument: ScpiInstrument, pipe: BasePipe) -> None:
        super().__init__()
        self._instrument = instrument
        self._pipe = pipe

    async def request(
        self,
        request: JsonType,
        timeout: float | None = None,
    ) -> JsonType:
        inner_req = {
            "instrument": self._instrument.to_json(),
            "request": request,
            "lock": self._pipe.lock_id,
        }
        if timeout is not None:
            inner_req["timeout"] = duration_to_json(timeout)
            timeout = timeout + self._pipe.timeout

        result = await self._pipe.get(
            {"Scpi": inner_req},
            timeout=timeout,
        )
        ComSrvError.check_raise(result)
        return result["Scpi"]


class PrologixTransport(Transport):
    def __init__(self, instrument: PrologixInstrument, pipe: BasePipe) -> None:
        super().__init__()
        self._instrument = instrument
        self._pipe = pipe

    async def request(
        self,
        request: JsonType,
        timeout: float | None = None,
    ) -> JsonType:
        inner_req = {
            "instrument": self._instrument.to_json(),
            "request": {
                "addr": self._instrument.gpib_address,
                "scpi": request,
            },
            "lock": self._pipe.lock_id,
        }
        if timeout is not None:
            inner_req["timeout"] = duration_to_json(self._pipe.timeout)
            timeout = timeout + self._pipe.timeout

        result = await self._pipe.get(
            {"Prologix": inner_req},
            timeout=timeout,
        )
        ComSrvError.check_raise(result)
        return result["Scpi"]


class ScpiPipeBase:
    async def query(self, msg: str, timeout: float | None = None) -> str:
        raise NotImplementedError

    async def write(self, msg: str) -> None:
        raise NotImplementedError

    async def query_binary(self, msg: str, timeout: float | None = None) -> bytes:
        raise NotImplementedError

    async def read_raw(self, timeout: float | None = None) -> bytes:
        raise NotImplementedError


class ScpiPipe(BasePipe, ScpiPipeBase):
    def __init__(
        self, instrument: Union[str, ScpiInstrument], rpc: Optional[Rpc] = None
    ):
        if isinstance(instrument, str):
            instrument = ScpiInstrument.parse(instrument)
        assert isinstance(instrument, ScpiInstrument)

        if isinstance(instrument, PrologixInstrument):
            transport = PrologixTransport(instrument, self)  # type: ignore
        else:
            transport = ScpiTransport(instrument, self)  # type: ignore
        self._transport: PrologixTransport | ScpiTransport = transport
        BasePipe.__init__(self, instrument.address, rpc)

    async def request(
        self,
        request: JsonType,
        timeout: float | None = None,
    ) -> JsonType:
        return await self._transport.request(request, timeout=timeout)

    async def query(self, msg: str, timeout: float | None = None) -> str:
        result = await self.request({"QueryString": msg}, timeout=timeout)
        if not isinstance(result, dict):
            raise ComSrvError("Unexpected response")
        return result["String"]  # type: ignore

    async def write(self, msg: str) -> None:
        await self.request({"Write": msg})

    async def query_binary(self, msg: str, timeout: float | None = None) -> bytes:
        result = await self.request({"QueryBinary": msg}, timeout=timeout)
        if not isinstance(result, dict):
            raise ComSrvError("Unexpected response")
        binary = result["Binary"]
        if not isinstance(binary, dict):
            raise ComSrvError("Unexpected response")
        data = binary["data"]
        return base64.b64decode(data)  # type: ignore

    async def read_raw(self, timeout: float | None = None) -> bytes:
        result = await self.request({"ReadRaw": None}, timeout=timeout)
        if not isinstance(result, dict):
            raise ComSrvError("Unexpected response")
        ComSrvError.check_raise(result)
        data = result["Binary"]["data"]  # type: ignore
        return base64.b64decode(data)  # type: ignore


class SerialScpiPipe(ScpiPipeBase):
    def __init__(
        self,
        bs_pipe: Union[str, ByteStreamInstrument, ByteStreamPipe],
        term: str = "\n",
        timeout: float = 1.0,
        rpc: Rpc | None = None,
    ) -> None:
        if isinstance(bs_pipe, str) or isinstance(bs_pipe, ByteStreamInstrument):
            bs_pipe = ByteStreamPipe(bs_pipe, rpc=rpc)
        assert len(term) == 1, "term must be a single character"
        self._inner = bs_pipe
        self._timeout = timeout
        self._term = term

    @property
    def timeout(self) -> float:
        return self._timeout

    @timeout.setter
    def timeout(self, value: float) -> None:
        self._timeout = float(value)

    @property
    def term(self) -> str:
        return self._term

    @term.setter
    def term(self, value: str) -> None:
        assert len(value) == 1
        self._term = value

    async def query(self, msg: str, timeout: None | float = None) -> str:
        timeout = timeout or self._timeout
        return await self._inner.query_line(msg, timeout, term=self._term)

    async def write(self, msg: str) -> None:
        return await self._inner.write_line(msg, term=self._term)

    async def read_raw(self, timeout: None | float = None) -> bytes:
        timeout = timeout or self._timeout
        ret = await self._inner.read_line(timeout, term=self._term)
        return ret.encode("utf-8")
