import asyncio
from typing import Any, Optional, Union
from broadcast_wsrpc import Client, JsonType, JsonDict
from . import (
    Address,
    BasePipe,
    ComSrvError,
    Instrument,
    Rpc,
    duration_to_json,
    get_default_ws_url,
)
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
    """
    Represents an instrument capable of operating on a bytestream
    """

    @classmethod
    def parse(cls, address_string: str) -> "ByteStreamInstrument":
        """
        Parse a resource string describing a bytestream capable instrument and construct the
        correspoding instrument.

        Supported strings are of the form:

         - `serial::<path-to-serial>::<baud-rate>::<settings>` - For serial ports
         - `ftdi::<serial-number>::<baud-rate>::<settings>` - For FTDI devices
         - `tcp::<host_or_ip>:<port>` - Connect to TCP socket
        """
        address_string = address_string.strip()
        if address_string.startswith("serial::"):
            address_string = address_string.replace("serial::", "", 1)
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
            address_string = address_string.replace("ftdi::", "", 1)
            m = FTDI_ADDRESS_RE.match(address_string)
            if m is None:
                raise ValueError(
                    "FTDI Address should be of the form `serial::<path>::<baudrate>::<config>`"
                )
            path, baudrate, config = cls._parse_serial_path(m)
            return FtdiInstrument(FtdiAddress(path), SerialPortConfig(config, baudrate))
        elif address_string.startswith("tcp::"):
            address_string = address_string.replace("tcp::", "", 1)
            m = TCP_ADDRESS_RE.match(address_string)
            assert m is not None
            host_or_ip = m.group("host_or_ip")
            port = int(m.group("port"))
            if port < 1 or port > 65535:
                raise ValueError("Port needs to be in range (1, 65535)")
            return TcpInstrument(TcpAddress(host_or_ip, port))
        raise ValueError(
            "Invalid address. Must start with `serial::` or `ftdi::` or `tcp::`"
        )

    @classmethod
    def _parse_serial_path(cls, match: re.Match) -> tuple[str, int, str]:
        baudrate = int(match.group("baudrate"))
        path = match.group("path")
        config = match.group("config")
        return (path, baudrate, config)


class FtdiAddress(ByteStreamAddress):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self) -> JsonDict:
        return {"port": self.port}

    @property
    def enum_name(self) -> str:
        return "Ftdi"


class SerialAddress(ByteStreamAddress):
    def __init__(self, port: str) -> None:
        self.port = port

    def to_json(self) -> JsonDict:
        return {"port": self.port}

    @property
    def enum_name(self) -> str:
        return "Serial"


class TcpAddress(ByteStreamAddress):
    def __init__(self, host: str, port: int) -> None:
        self.port = port
        self.host = host

    def to_json(self) -> JsonDict:
        return {"port": self.port, "host": self.host}

    @property
    def enum_name(self) -> str:
        return "Tcp"


class TcpInstrument(ByteStreamInstrument):
    def __init__(self, address: TcpAddress) -> None:
        super().__init__()
        self._address = address

    @property
    def address(self) -> Address:
        return self._address

    def to_json(self) -> JsonDict:
        return {"Tcp": {"address": self.address.to_json()}}


class SerialPortConfig(object):
    def __init__(self, config: str, baudrate: int) -> None:
        self.config = config
        self.baudrate = baudrate

    def to_json(self) -> JsonDict:
        return {"config": self.config, "baudrate": self.baudrate}


class FtdiInstrument(ByteStreamInstrument):
    def __init__(self, address: FtdiAddress, port_config: SerialPortConfig) -> None:
        super().__init__()
        self._address = address
        self._port_config = port_config

    def to_json(self) -> JsonDict:
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

    def to_json(self) -> JsonDict:
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
    """
    Provides a communication interface that operates on bidirectional byte streams.
    The underlying transport could be a TCP connection or a serial port.
    The constructor accepts resource strings to describe the instrument and its configuration:

    - `serial::<path-to-serial>::<baud-rate>::<settings>` - For serial ports
    - `ftdi::<serial-number>::<baud-rate>::<settings>` - For FTDI devices
    - `tcp::<host_or_ip>:<port>` - Connect to TCP socket

    :param instrumnt: Either a `ByteStreamInstrument` or a resource string describing an appropriate
        instrument.
    :param rpc: An optional RPC service. Refer to `BasePipe` for details on the default implementation.
    """

    def __init__(
        self, instrument: Union[str, ByteStreamInstrument], rpc: Optional[Rpc] = None
    ):
        if not isinstance(instrument, ByteStreamInstrument):
            instrument = ByteStreamInstrument.parse(instrument)
        assert isinstance(instrument, ByteStreamInstrument)
        self._instrument = instrument
        super().__init__(instrument.address, rpc)

    @property
    def instrument(self) -> ByteStreamInstrument:
        return self._instrument

    @property
    def address(self) -> Address:
        return self._instrument.address

    async def request(
        self,
        request: JsonType,
        timeout: float | None = None,
    ) -> JsonDict:
        """
        Send an RPC request to the bytestream handler of the RPC protocol.
        """
        result = await self.get(
            {
                "Bytes": {
                    "instrument": self._instrument.to_json(),
                    "request": request,
                    "lock": self._lock,
                }
            },
            timeout=timeout,
        )
        ComSrvError.check_raise(result)
        return result["Bytes"]

    async def write(self, data: bytes) -> None:
        """
        Write the bytes in `data` to the stream.
        """
        await self.request({"Write": list(data)})

    async def read_all(self) -> bytes:
        """
        Read all data in the buffer of the stream and returns without
        blocking on the stream.
        """
        result = await self.request("ReadAll")
        data = bytes(result["Data"])
        return data

    async def read_to_term(self, term: int, timeout: float) -> bytes:
        """
        Read from the stream until the termination character is found.
        """
        result = await self.request(
            {"ReadToTerm": {"term": term, "timeout": duration_to_json(timeout)}},
            timeout=timeout + self._timeout,
        )
        data = bytes(result["Data"])
        return data

    async def read_exact(self, count: int, timeout: float) -> bytes:
        """
        Read exactly `count` byte from the stream.
        """
        result = await self.request(
            {"ReadExact": {"count": count, "timeout": duration_to_json(timeout)}},
            timeout=timeout + self._timeout,
        )
        data = bytes(result["Data"])
        return data

    async def cobs_write(self, data: bytes) -> None:
        """
        Apply the COBS framing to the provided `data` and write it to the stream.
        """
        await self.request({"CobsWrite": list(data)})

    async def cobs_read(self, timeout: float) -> bytes:
        """
        Read a COBS encoded frame from stream.
        """
        result = await self.request(
            {"CobsRead": duration_to_json(timeout)},
            timeout=timeout + self._timeout,
        )
        data = bytes(result["Data"])
        return data

    async def cobs_query(self, data: bytes, timeout: float) -> bytes:
        """
        This is a combination of `cobs_write` followed by a `cobs_read` call.
        """
        result = await self.request(
            {"CobsQuery": {"data": list(data), "timeout": duration_to_json(timeout)}},
            timeout=timeout + self._timeout,
        )
        data = bytes(result["Data"])
        return data

    async def write_line(self, line: str, term: str | int = "\n") -> None:
        """
        Write a string terminated with `term` to the stream.
        """
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        await self.request({"WriteLine": {"line": line, "term": term}})

    async def read_line(self, timeout: float, term: Union[int, str] = "\n") -> str:
        """
        Read a string terminated with `term` from the stream.
        """
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await self.request(
            {"ReadLine": {"term": term, "timeout": duration_to_json(timeout)}},
            timeout=timeout + self._timeout,
        )
        return result["String"]

    async def query_line(
        self,
        line: str,
        timeout: float,
        term: Union[int, str] = "\n",
    ) -> str:
        """
        This is a combination of `write_line` followed by a `read_line` call.
        """
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
            },
            timeout=timeout + self._timeout,
        )
        return result["String"]

    async def disconnect(self) -> None:
        """
        Disconnect the underlying handle.
        """
        await self.request("Disconnect")

    async def connect(self) -> None:
        """
        Connect the underlying handle
        """
        await self.request("Connect")


class CobsStream:
    def __init__(
        self,
        instrument: Union[ByteStreamInstrument, str],
        use_crc: bool,
        maxsize: int = 0,
        client: Optional[Client] = None,
    ) -> None:
        if not isinstance(instrument, ByteStreamInstrument):
            instrument = ByteStreamInstrument.parse(instrument)
        self._instrument = instrument
        self._use_crc = use_crc
        self._receiver_task: asyncio.Task[None] | None = None
        if client is None:
            client = Client()
        self._client = client
        self._receiver: asyncio.Queue = asyncio.Queue(maxsize=maxsize)
        self.receiver_overflow = False

    async def connect(self, url: str | None = None, **kw: Any) -> None:
        if self._client.connected:
            return
        if url is None:
            url = get_default_ws_url()
        await self._client.connect(url, **kw)

    async def start(self) -> None:
        await self.connect()
        self._receiver_task = asyncio.create_task(self._receive_loop())
        await self.rpc({"Start": {"use_crc": self._use_crc}})

    async def rpc(self, request: JsonType) -> JsonDict:
        await self.connect()
        resp = await self._client.request(
            {
                "CobsStream": {
                    "instrument": self._instrument.to_json(),
                    "request": request,
                }
            }
        )
        if not isinstance(resp, dict):
            raise ComSrvError("Unexpected response")
        ComSrvError.check_raise(resp)
        if "CobsStream" not in resp:
            raise ComSrvError("Unexpected wire format")
        return resp["CobsStream"]

    async def _receive_loop(self) -> None:
        def filter(x: JsonType) -> JsonDict | None:
            if not isinstance(x, dict):
                return None
            # TODO: this should also filter on the instrument
            if "CobsStream" not in x:
                return None
            x = x["CobsStream"]
            if not isinstance(x, dict):
                return None
            if "MessageReceived" not in x:
                return None
            return x["MessageReceived"]

        rx = self._client.notifications().map(filter)
        with rx:
            while True:
                msg = await rx.next()
                data = msg["data"]
                msg = bytes(data)  # type: ignore
                try:
                    self._receiver.put_nowait(msg)
                except asyncio.QueueFull:
                    self.receiver_overflow = True

    async def send(self, data: bytes) -> None:
        await self.rpc({"SendFrame": {"data": list(data)}})

    async def stop(self) -> None:
        await self.rpc({"Stop": None})

    async def close(self, stop: bool = True) -> None:
        if stop:
            await self.stop()
        if self._receiver_task is not None:
            self._receiver_task.cancel()
        await self._client.disconnect()

    @property
    def receiver(self) -> asyncio.Queue[bytes]:
        return self._receiver
