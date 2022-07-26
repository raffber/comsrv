"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

import json
from argparse import ArgumentError
from enum import Enum
from math import prod
from typing import List, Optional, Union

from aiohttp import ClientSession
from pywsrpc.client import Client


class ComSrvError(Exception):
    """
    Base exception for all comsrv errors.
    All errors occuring on the remote comsrv will throw this error.
    """

    @classmethod
    def parse(cls, data):
        if "Protocol" in data:
            return ProtocolError.parse(data["Protocol"])
        if "Transport" in data:
            return TransportError(data["Transport"])
        if "Argument" in data:
            return ArgumentError(message=data["Argument"])
        if "Internal" in data:
            return InternalError(message=data["Internal"])
        return ComSrvError(data)

    @classmethod
    def check_raise(cls, result):
        if "Error" in result:
            raise ComSrvError.parse(result["Error"])


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
    def parse(cls, data):
        if "Timeout" in data:
            return ProtocolTimeoutError()
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


def setup_default(host=None, http_port=None, ws_port=None):
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


def get_default_http_url():
    """
    Returns the default HTTP URL to connect to when constructing
    an `HttpRpc`.
    """
    return "http://{}:{}".format(_default_host, _default_http_port)


def get_default_ws_url():
    """
    Returns the default websocket URL to connect to when
    constructing a `WsRpc`.
    """
    return "ws://{}:{}".format(_default_host, _default_ws_port)


async def connect_websocket_rpc(url=None) -> Client:
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

    async def get(self, request, timeout):
        """
        Send a request and wait for the response, but for at most `timeout`.
        """
        raise NotImplementedError

    @classmethod
    def make_default(cls):
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

    def __init__(self, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url

    async def get(self, request, timeout):
        data = json.dumps(request).encode()
        async with ClientSession(timeout=timeout) as session:
            async with session.get(self._url, data=data) as resp:
                data = json.loads(await resp.text())
                if resp.status != 200:
                    raise ComSrvError(data)
                return data


class WsRpc(Rpc):
    """
    RPC service implementation using WebSockets as transport
    """

    def __init__(self, url=None):
        if url is None:
            url = get_default_ws_url()
        self._url = url
        self._client = Client()

    async def get(self, data, timeout):
        await self.connect(self._url)
        return await self._client.request(data, timeout)

    async def connect(self, url=None):
        """
        Connect to the remote server
        """
        return await self._client.connect(url)


class Address(object):
    """
    Base class to represent instrument addresses.
    """

    def to_json(self):
        raise NotImplementedError

    def to_json_enum(self):
        return {self.enum_name: self.to_json()}

    @property
    def enum_name(self):
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

    def to_json(self):
        raise NotImplementedError


def duration_to_json(time_in_seconds: float):
    """
    Serialize a duration in seconds to a RPC `Duration` object.
    """
    micros = int((time_in_seconds % 1.0) * 1000000)
    seconds = round(time_in_seconds)
    return {"micros": micros, "seconds": seconds}


class BasePipe(object):
    """
    Base class implementing functionality common to all instruments.

    :param address: The address to connect to
    """

    DEFAULT_TIMEOUT = 1.0

    def __init__(self, address: Address, rpc: Optional[Rpc] = None):
        if rpc is None:
            rpc = Rpc.make_default()
        self._lock_time = 1.0
        self._address = address
        self._lock = None
        self._rpc = rpc
        self._timeout = BasePipe.DEFAULT_TIMEOUT

    @property
    def rpc(self):
        return self._rpc

    @property
    def timeout(self):
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        self._timeout = float(value)

    @property
    def address(self):
        return self._address

    @property
    def lock_time(self):
        """
        Return the default lock time for an instrument.
        This lock time is used when using the pipe as an async context manager.
        :return:
        """
        return self._lock_time

    @lock_time.setter
    def lock_time(self, value: float):
        """
        Setup the default lock time for an instrument.
        This lock time is used when using the pipe as an async context manager.
        """
        self._lock_time = value

    @property
    def locked(self):
        """
        Return True if a lock has been acquired
        """
        return self._lock is not None

    async def lock(self, timeout: Union[float, None] = None):
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
            }
        )
        self._lock = reply["Locked"]["lock_id"]
        return self

    @property
    def lock_id(self) -> Optional[str]:
        return self._lock

    async def get(self, data, timeout=None):
        """
        Send a request and return the corresponding response but wait for at most `timeout`
        seconds.
        If `timeout` is not specified, `self.timeout` applies.
        """
        if timeout is None:
            timeout = self._timeout
        return await self._rpc.get(data, timeout)

    async def __aenter__(self):
        """
        Lock the instrument
        :return: self
        """
        return await self.lock()

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """
        Unlock the instrument
        """
        await self.unlock()

    async def unlock(self):
        """
        Unlock the instrument
        :return: self
        """
        if self._lock is None:
            return
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

    async def drop(self):
        """
        Drop and disconnect the instrument.
        """
        await ComSrv(rpc=self._rpc).drop(self.address.to_json_enum(), self._lock)


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

    def to_address(self, baudrate: int, params: str = "8N1"):
        raise NotImplementedError


class CanDriverType(Enum):
    PCAN = "pcan"
    SOCKETCAN = "socket"


class CanDevice(object):
    def __init__(self, interface_name: str, driver_type: CanDriverType) -> None:
        self.driver_type = driver_type
        self.interface_name = interface_name

    def to_address(self, bitrate=None):
        raise NotImplementedError


class ComSrv(object):
    DEFAULT_TIMEOUT = 1.0

    def __init__(self, rpc=None, timeout=DEFAULT_TIMEOUT):
        if rpc is None:
            rpc = HttpRpc.make_default()
        self._rpc = rpc
        self._timeout = timeout

    @property
    def timeout(self):
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        self._timeout = value

    @property
    def rpc(self):
        return self._rpc

    async def get(self, data, timeout=None):
        if timeout is None:
            timeout = self._timeout
        return await self._rpc.get(data, timeout)

    async def drop(self, addr, lock=None):
        result = await self.get({"Drop": {"addr": addr.to_json_enum(), "id": lock}})
        ComSrvError.check_raise(result)

    async def drop_all(self):
        result = await self.get({"DropAll": None})
        ComSrvError.check_raise(result)

    async def shutdown(self):
        result = await self.get({"Shutdown": None})
        ComSrvError.check_raise(result)

    async def list_connected_instruments(self):
        result = await self.get({"ListConnectedInstruments": None})
        ComSrvError.check_raise(result)
        return result["Instruments"]

    async def list_hid_devices(self):
        from .hid import enumerate_hid_devices

        return await enumerate_hid_devices(rpc=self.rpc)

    async def list_serial_ports(self):
        result = await self.get({"ListSerialPorts": None})
        ComSrvError.check_raise(result)
        return result["SerialPorts"]

    async def list_ftdis(self) -> List[FtdiDeviceInfo]:
        result = await self.get(
            {
                "ListFtdiDevices": None,
            }
        )
        ComSrvError.check_raise(result)
        ret = []
        for x in result["FtdiDevices"]:
            ret.append(FtdiDeviceInfo(**x))
        return ret

    async def list_can_devices(self) -> List[CanDevice]:
        result = await self.get(
            {
                "ListCanDevices": None,
            }
        )
        ComSrvError.check_raise(result)
        ret = []
        for x in result["CanDevices"]:
            if x["driver_type"] == "SocketCAN":
                driver_type = CanDriverType.SOCKETCAN
            elif x["driver_type"] == "PCAN":
                driver_type = CanDriverType.PCAN
            ret.append(CanDevice(x["interface_name"], driver_type=driver_type))
        return ret


from .bytestream import (
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
from .can import CanBus
from .hid import HidDevice, enumerate_hid_devices
from .modbus import ModBusDevice
from .scpi import ScpiPipe, SerialScpiPipe
from .sigrok import SigrokDevice
