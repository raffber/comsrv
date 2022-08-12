import base64
from typing import Optional, Union

from . import Address, ComSrvError, BasePipe, Instrument, Rpc
from .bytestream import ByteStreamInstrument, ByteStreamPipe, SerialAddress


class ScpiAddress(Address):
    pass


class VxiAddress(ScpiAddress):
    def __init__(self, host: str):
        self.host = host
        super().__init__()

    def to_json(self):
        return self.host

    @property
    def enum_name(self):
        return "Vxi"


class VisaAddress(ScpiAddress):
    def __init__(self, visa_address_string: str):
        super().__init__()
        self.visa_address_string = visa_address_string

    def to_json(self):
        return self.visa_address_string

    @property
    def enum_name(self):
        return "Visa"


class ScpiInstrument(Instrument):
    def __init__(self, address: ScpiAddress):
        self._address = address

    @property
    def address(self):
        return self._address

    @classmethod
    def parse(cls, instrument: str):
        if instrument.startswith("vxi::"):
            splits = instrument.split("::")
            if len(splits) != 2:
                raise ValueError(
                    "Vxi address expected in the form `vxi::<host-name-or-ip>`"
                )
            return VxiInstrument(VxiAddress(splits[1]))
        if instrument.startswith("visa::"):
            instrument = instrument.removeprefix("visa::")
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
    def to_json(self):
        return {"Vxi": {"host": self._address.to_json()}}


class VisaInstrument(ScpiInstrument):
    def to_json(self):
        return {"Visa": {"address": self._address.to_json()}}


class PrologixInstrument(ScpiInstrument):
    def __init__(self, address: SerialAddress, gpib_address: int) -> None:
        self._gpib_address = gpib_address
        super().__init__(address)

    @property
    def gpib_address(self):
        return self._gpib_address

    def to_json(self):
        return {"address": self._address.to_json()}


class Transport(object):
    async def request(self, request):
        raise NotImplementedError


class ScpiTransport(Transport):
    def __init__(self, instrument: ScpiInstrument, pipe: BasePipe) -> None:
        super().__init__()
        self._instrument = instrument
        self._pipe = pipe

    async def request(self, request):
        result = await self._pipe.get(
            {
                "Scpi": {
                    "instrument": self._instrument.to_json(),
                    "request": request,
                    "lock": self._pipe.lock_id,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Scpi"]


class PrologixTransport(Transport):
    def __init__(self, instrument: PrologixInstrument, pipe: BasePipe) -> None:
        super().__init__()
        self._instrument = instrument
        self._pipe = pipe

    async def request(self, request):
        result = await self._pipe.get(
            {
                "Prologix": {
                    "instrument": self._instrument.to_json(),
                    "request": {
                        "addr": self._instrument.gpib_address,
                        "scpi": request,
                    },
                    "lock": self._pipe.lock_id,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Scpi"]


class ScpiPipe(BasePipe):
    def __init__(
        self, instrument: Union[str, ScpiInstrument], rpc: Optional[Rpc] = None
    ):
        if isinstance(instrument, str):
            instrument = ScpiInstrument.parse(instrument)
        if isinstance(instrument, PrologixInstrument):
            self._transport = PrologixTransport(instrument, self)
        else:
            self._transport = ScpiTransport(instrument, self)
        super().__init__(instrument.address, rpc)

    async def request(self, request):
        return await self._transport.request(request)

    async def query(self, msg: str) -> str:
        result = await self.request({"QueryString": msg})
        return result["String"]

    async def write(self, msg: str):
        await self.request({"Write": msg})

    async def query_binary(self, msg: str) -> bytes:
        result = await self.request({"QueryBinary": msg})
        data = result["Binary"]["data"]
        return base64.b64decode(data)

    async def read_raw(self) -> bytes:
        result = await self.get("ReadRaw")
        ComSrvError.check_raise(result)
        data = result["Binary"]["data"]
        return base64.b64decode(data)


class SerialScpiPipe(BasePipe):
    def __init__(self, bs_pipe: Union[str, ByteStreamPipe], term="\n", timeout=1.0):
        if isinstance(bs_pipe, str):
            bs_pipe = ByteStreamPipe(bs_pipe)
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
