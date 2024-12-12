"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

from dataclasses import dataclass
import json
from enum import Enum
from math import floor
from typing import Any, List, Optional, Tuple, Union

from aiohttp import ClientSession, ClientTimeout  # type: ignore
from broadcast_wsrpc.client import Client
from broadcast_wsrpc.json import JsonObject, JsonType


class ComSrvError(Exception):
    """
    Base exception for all comsrv errors.
    All errors occuring on the remote comsrv will throw this error.
    """

    @classmethod
    def parse(cls, data: JsonType) -> "ComSrvError":
        if not isinstance(data, dict):
            return ComSrvError(data)
        if "Protocol" in data:
            protocol = data["Protocol"]
            assert isinstance(protocol, dict)
            return ProtocolError.parse(protocol)
        if "Transport" in data:
            return TransportError(data["Transport"])
        if "Argument" in data:
            return ArgumentError(data["Argument"])
        if "Internal" in data:
            return InternalError(data["Internal"])
        return ComSrvError(data)

    @classmethod
    def check_raise(cls, result: JsonObject) -> None:
        if "Error" in result:
            err = result["Error"]
            raise ComSrvError.parse(err)


class ArgumentError(ComSrvError):
    """
    Captures all errors that occur in case a transport fails e.g. a TCP connection drops.
    """

    pass


class TransportError(ComSrvError):
    """
    Captures all errors that occur in case a transport fails e.g. a TCP connection drops.
    """

    pass


class ProtocolError(ComSrvError):
    """
    Captures all errors that occur on protocol-level (as opposed to on transport level).
    For example if a remote side sends an invalid ModBus-frame.
    """

    @classmethod
    def parse(cls, data: JsonObject) -> "ProtocolError":
        if "Timeout" in data:
            return ProtocolTimeoutError()
        if "Other" in data:
            other = data["Other"]
            assert isinstance(other, dict)
            return ProtocolError(
                other["description"],
                other["backtrace"],
            )
        return ProtocolError(data)


class ProtocolTimeoutError(ProtocolError):
    """
    Occurs if an operation times out on protocol level.
    For example if a device fails to reply in time.
    """

    pass


class InternalError(ComSrvError):
    """
    Thrown in case an internal error occurs in the comsrv. This points to a bug in the service.
    """

    pass


_default_ws_port = 5902
_default_http_port = 5903
_default_host = "127.0.0.1"


def setup_default(
    host: str | None = None,
    http_port: int | None = None,
    ws_port: int | None = None,
) -> None:
    """
    Allows setting up default host and port to connect to if
    no more specific parameters are provided to the creation
    of `Rpc` services.

    This function is particularly useful when interactively working on a remote
    `comsrv`. Instead of specifing the `rpc=` argument for every
    call one may use this function to configure a remote host
    to be used everywhere.

    Note that `Rpc` objects are default constructed if no
    `rpc=` parameter is passed to most function.
    """
    global _default_host
    global _default_http_port
    global _default_ws_port
    if host is None:
        host = _default_host
    if http_port is None:
        http_port = _default_http_port
    if ws_port is None:
        ws_port = _default_ws_port
    _default_host = host
    _default_http_port = http_port
    _default_ws_port = ws_port


def get_default_http_url() -> str:
    """
    Returns the default HTTP URL to connect to when constructing
    an `HttpRpc`.
    """
    return "http://{}:{}".format(_default_host, _default_http_port)


def get_default_ws_url() -> str:
    """
    Returns the default websocket URL to connect to when
    constructing a `WsRpc`.
    """
    return "ws://{}:{}".format(_default_host, _default_ws_port)


async def connect_websocket_rpc(url: str | None = None) -> Client:
    """
    Construct and connect a `wsrpc.Client` to the specified url.

    :param url: If `url` is None the default websocket URL is used.
    """
    if url is None:
        url = get_default_ws_url()
    return await Client().connect(url)


class Rpc(object):
    """
    Base class for RPC service implementations.
    """

    async def get(self, request: JsonType, timeout: float) -> JsonObject:
        """
        Send a request and wait for the response, but for at most `timeout`.
        """
        raise NotImplementedError

    @classmethod
    def make_default(cls) -> "Rpc":
        """
        Create a default RPC service if no other information is provided.
        """
        return HttpRpc()


class HttpRpc(Rpc):
    """
    RPC service implementation using HTTP as transport

    This is somewhat slower than using websockets, however
    has the advantage that it is completely stateless.
    If speed is not a concern, this should be the default choice.
    """

    def __init__(self, url: str | None = None) -> None:
        if url is None:
            url = get_default_http_url()
        self._url = url

    async def get(self, request: JsonType, timeout: float) -> JsonObject:
        data = json.dumps(request).encode()
        to = ClientTimeout(total=None, sock_connect=timeout, sock_read=timeout)
        async with ClientSession(timeout=to) as session:
            async with session.get(self._url, data=data) as resp:
                json_data = json.loads(await resp.text())
                if resp.status != 200:
                    raise ComSrvError(data)
                return json_data

    @classmethod
    def make_default(cls) -> "Rpc":
        """
        Create a default RPC service if no other information is provided.
        """
        return HttpRpc()


class WsRpc(Rpc):
    """
    RPC service implementation using WebSockets as transport

    :param kw: Passed to `broadcast_wsrpc.client.Client.connect()`
    """

    def __init__(self, url: str | None = None, **kw: Any) -> None:
        if url is None:
            url = get_default_ws_url()
        self._url = url
        self._kw = kw
        self._client = Client()

    async def get(self, request: JsonType, timeout: float) -> JsonObject:
        await self.connect(self._url)
        ret = await self._client.request(request, timeout)
        if not isinstance(ret, dict):
            raise ComSrvError(f"Got wrong JSON type. Expected dict, got {type(ret)}")
        return ret

    async def connect(self, url: str | None = None) -> "WsRpc":
        """
        Connect to the remote server
        """
        if url is None:
            url = self._url
        await self._client.connect(url, **self._kw)
        return self

    @classmethod
    def make_default(cls) -> "Rpc":
        """
        Create a default RPC service if no other information is provided.
        """
        return WsRpc()


class Address(object):
    """
    Base class to represent instrument addresses.
    """

    def to_json(self) -> JsonType:
        raise NotImplementedError

    def to_json_enum(self) -> JsonObject:
        return {self.enum_name: self.to_json()}

    @property
    def enum_name(self) -> str:
        raise NotImplementedError


class Instrument(object):
    """
    Base class to represent instrument configurations.
    An instrument combines an `Address` with additional instrument configuration
    such as a baudrate in case of serial port.
    """

    @property
    def address(self) -> Address:
        raise NotImplementedError

    def to_json(self) -> JsonType:
        raise NotImplementedError


def duration_to_json(time_in_seconds: float) -> JsonObject:
    """
    Serialize a duration in seconds to a RPC `Duration` object.
    """
    micros = int((time_in_seconds % 1.0) * 1000000)
    seconds = floor(time_in_seconds)
    return {"micros": micros, "seconds": seconds}


class BasePipe:
    """
    Base class implementing functionality common to all instruments.

    :param address: The address to connect to
    """

    DEFAULT_TIMEOUT = 2.0

    def __init__(self, address: Address, rpc: Optional[Rpc] = None):
        if rpc is None:
            rpc = Rpc.make_default()
        self._lock_time = 1.0
        self._address = address
        self._lock: str | None = None
        self._rpc = rpc
        self._timeout = BasePipe.DEFAULT_TIMEOUT

    @property
    def rpc(self) -> Rpc:
        return self._rpc

    @property
    def timeout(self) -> float:
        return self._timeout

    @timeout.setter
    def timeout(self, value: float) -> None:
        self._timeout = float(value)

    @property
    def address(self) -> Address:
        return self._address

    @property
    def lock_time(self) -> float:
        """
        Return the default lock time for an instrument.
        This lock time is used when using the pipe as an async context manager.
        :return:
        """
        return self._lock_time

    @lock_time.setter
    def lock_time(self, value: float) -> None:
        """
        Setup the default lock time for an instrument.
        This lock time is used when using the pipe as an async context manager.
        """
        self._lock_time = value

    @property
    def locked(self) -> bool:
        """
        Return True if a lock has been acquired
        """
        return self._lock is not None

    async def lock(self, timeout: Union[float, None] = None) -> "BasePipe":
        """
        Lock the instrument for a certain time. If the timeout is specified as `None`,
        the value of `self.lock_time` is used.

        :param timeout: The time to lock the instrument in seconds
        :return: self
        """
        await self.unlock()
        lock_time = timeout or self._lock_time
        reply = await self.get(
            {
                "Lock": {
                    "addr": self.address.to_json_enum(),
                    "timeout": duration_to_json(lock_time),
                }
            },
            timeout=lock_time,
        )
        lock = reply["Locked"]
        assert isinstance(lock, dict) and "lock_id" in lock
        lock_id = lock["lock_id"]
        assert isinstance(lock_id, str)
        self._lock = lock_id
        return self

    @property
    def lock_id(self) -> str | None:
        return self._lock

    async def get(self, data: JsonType, timeout: float | None = None) -> JsonObject:
        """
        Send a request and return the corresponding response but wait for at most `timeout`
        seconds.
        If `timeout` is not specified, `self.timeout` applies.
        """
        if timeout is None:
            timeout = self._timeout
        return await self._rpc.get(data, timeout)

    async def __aenter__(self) -> "BasePipe":
        """
        Lock the instrument
        :return: self
        """
        return await self.lock()

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        """
        Unlock the instrument
        """
        await self.unlock()

    async def unlock(self) -> "BasePipe":
        """
        Unlock the instrument
        :return: self
        """
        if self._lock is None:
            return self
        await self.get(
            {
                "Unlock": {
                    "addr": self.address.to_json_enum(),
                    "id": self._lock,
                }
            }
        )
        self._lock = None
        return self

    async def drop(self) -> None:
        """
        Drop and disconnect the instrument.
        """
        await ComSrv(rpc=self._rpc).drop(self.address, self._lock)


class FtdiDeviceInfo(object):
    def __init__(
        self,
        port_open: bool,
        vendor_id: int,
        product_id: int,
        serial_number: str,
        description: str,
    ) -> None:
        self.port_open = port_open
        self.vendor_id = vendor_id
        self.product_id = product_id
        self.serial_number = serial_number
        self.description = description

    def to_address(self, baudrate: int, params: str = "8N1") -> "FtdiInstrument":
        return FtdiInstrument(
            FtdiAddress(self.serial_number), SerialPortConfig(params, baudrate)
        )


@dataclass
class HidDeviceInfo:
    vid: int
    pid: int
    manufacturer: Optional[str] = None
    product: Optional[str] = None
    serial_number: Optional[str] = None


class CanDriverType(Enum):
    PCAN = "pcan"
    SOCKETCAN = "socket"


class CanDevice(object):
    def __init__(self, interface_name: str, driver_type: CanDriverType) -> None:
        self.driver_type = driver_type
        self.interface_name = interface_name

    def to_address(self, bitrate: int | None = None) -> "Address":
        raise NotImplementedError


class ComSrv(object):
    DEFAULT_TIMEOUT = 1.0

    def __init__(
        self, rpc: Rpc | None = None, timeout: float = DEFAULT_TIMEOUT
    ) -> None:
        if rpc is None:
            rpc = WsRpc.make_default()
        self._rpc = rpc
        self._timeout = timeout

    @property
    def timeout(self) -> float:
        return self._timeout

    @timeout.setter
    def timeout(self, value: float) -> None:
        self._timeout = value

    @property
    def rpc(self) -> Rpc:
        return self._rpc

    async def get(self, data: JsonType, timeout: float | None = None) -> JsonObject:
        if timeout is None:
            timeout = self._timeout
        return await self._rpc.get(data, timeout)

    async def drop(self, addr: Address, lock: str | None = None) -> None:
        result = await self.get({"Drop": {"addr": addr.to_json_enum(), "id": lock}})
        ComSrvError.check_raise(result)

    async def get_version(self) -> Tuple[int, int, int]:
        result = await self.get({"Version": None})
        ComSrvError.check_raise(result)
        version = result["Version"]
        assert isinstance(version, dict)
        major = version["major"]
        minor = version["minor"]
        build = version["build"]
        assert (
            isinstance(major, int) and isinstance(minor, int) and isinstance(build, int)
        )
        return (major, minor, build)

    async def drop_all(self) -> None:
        result = await self.get({"DropAll": None})
        ComSrvError.check_raise(result)

    async def shutdown(self) -> None:
        result = await self.get({"Shutdown": None})
        ComSrvError.check_raise(result)

    async def list_connected_instruments(self) -> List[JsonObject]:
        result = await self.get({"ListConnectedInstruments": None})
        ComSrvError.check_raise(result)
        instruments = result["Instruments"]
        assert isinstance(instruments, list)
        return instruments  # type: ignore

    async def list_hid_devices(self) -> List[HidDeviceInfo]:
        from .hid import enumerate_hid_devices

        return await enumerate_hid_devices(rpc=self.rpc)

    async def list_serial_ports(self) -> List[str]:
        result = await self.get({"ListSerialPorts": None})
        ComSrvError.check_raise(result)
        serial_ports = result["SerialPorts"]
        assert isinstance(serial_ports, list)
        return serial_ports  # type: ignore

    async def list_ftdis(self) -> List[FtdiDeviceInfo]:
        result = await self.get({"ListFtdiDevices": None})
        ComSrvError.check_raise(result)
        ret = []
        ftdi_devices = result["FtdiDevices"]
        assert isinstance(ftdi_devices, list)
        for x in ftdi_devices:
            ret.append(FtdiDeviceInfo(**x))  # type: ignore
        return ret  # type: ignore

    async def list_can_devices(self) -> List[CanDevice]:
        result = await self.get(
            {
                "ListCanDevices": None,
            }
        )
        ComSrvError.check_raise(result)
        ret = []
        can_devices = result["CanDevices"]
        assert isinstance(can_devices, list)
        for x in can_devices:
            if x["driver_type"] == "SocketCAN":  # type: ignore
                driver_type = CanDriverType.SOCKETCAN
            elif x["driver_type"] == "PCAN":  # type: ignore
                driver_type = CanDriverType.PCAN
            ret.append(CanDevice(x["interface_name"], driver_type=driver_type))  # type: ignore
        return ret  # type: ignore


from .bytestream import (  # noqa: E402
    ByteStreamAddress,
    ByteStreamInstrument,
    ByteStreamPipe,
    FtdiAddress,
    FtdiInstrument,
    SerialAddress,
    SerialInstrument,
    SerialPortConfig,
    TcpAddress,
    TcpInstrument,
)
from .can import CanBus  # noqa: E402
from .hid import HidDevice, enumerate_hid_devices  # noqa: E402
from .modbus import ModBusDevice  # noqa: E402
from .scpi import ScpiPipe, SerialScpiPipe  # noqa: E402
from .sigrok import SigrokDevice  # noqa: E402

__all__ = [
    "ByteStreamAddress",
    "ByteStreamInstrument",
    "ByteStreamPipe",
    "FtdiAddress",
    "FtdiInstrument",
    "SerialAddress",
    "SerialInstrument",
    "SerialPortConfig",
    "TcpAddress",
    "TcpInstrument",
    "CanBus",
    "HidDevice",
    "enumerate_hid_devices",
    "ModBusDevice",
    "ScpiPipe",
    "SerialScpiPipe",
    "SigrokDevice",
]
