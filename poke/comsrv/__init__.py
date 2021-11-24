"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

import json
from typing import Union, Optional

from aiohttp import ClientSession

from pywsrpc.client import Client


class ComSrvError(Exception):

    @classmethod
    def parse(cls, data):
        if 'Hid' in data:
            return HidError(data['Hid'])
        if 'Timeout' in data:
            return TimeoutError()
        if 'Can' in data:
            return CanError()
        return ComSrvError(data)

    @classmethod
    def check_raise(cls, result):
        if 'Error' in result:
            raise ComSrvError.parse(result['Error'])


class TimeoutError(ComSrvError):
    pass


class HidError(ComSrvError):
    pass


class CanError(ComSrvError):
    pass


_default_ws_port = 5902
_default_http_port = 5903
_default_host = '127.0.0.1'


def setup_default(host=None, http_port=None, ws_port=None):
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
    return 'http://{}:{}'.format(_default_host, _default_http_port)


def get_default_ws_url():
    return 'ws://{}:{}'.format(_default_host, _default_ws_port)


async def connect_websocket_rpc(url=None) -> Client:
    if url is None:
        url = get_default_ws_url()
    return await Client().connect(url)


class Rpc(object):
    async def get(self, data, timeout):
        raise NotImplementedError


class HttpRpc(Rpc):
    def __init__(self, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url

    async def get(self, data, timeout):
        # TODO: timeout
        return await get(self._url, data)


class WsRpc(Rpc):
    def __init__(self, url=None):
        if url is None:
            url = get_default_ws_url()
        self._url = url
        self._client = Client()

    async def get(self, data, timeout):
        await self.connect(self._url)
        return await self._client.request(data, timeout)

    async def connect(self, url=None):
        return await self._client.connect(url)


async def get(url, data):
    data = json.dumps(data).encode()
    async with ClientSession() as session:
        async with session.get(url, data=data) as resp:
            data = json.loads(await resp.text())
            if resp.status != 200:
                raise ComSrvError(data)
            return data


class BasePipe(object):
    DEFAULT_TIMEOUT = 1.0

    def __init__(self, addr, rpc: Optional[Client] = None):
        if rpc is None:
            rpc = HttpRpc()
        self._addr = addr
        self._lock_time = 1.0
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
    def addr(self):
        return self._addr

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
        reply = await self.get({'Lock': {
            'addr': self._addr,
            'timeout_ms': int(lock_time * 1000),
        }})
        self._lock = reply['Locked']['lock_id']
        return self

    async def get(self, data, timeout=None):
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
        await self.get({'Unlock': self._lock})
        self._lock = None
        return self

    async def drop(self):
        await ComSrv(rpc=self._rpc).drop(self._addr)


class ComSrv(object):
    DEFAULT_TIMEOUT = 1.0

    def __init__(self, rpc=None, timeout=DEFAULT_TIMEOUT):
        if rpc is None:
            rpc = HttpRpc()
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

    async def drop(self, addr):
        result = await self.get({
            'Drop': addr
        })
        if 'Error' in result:
            raise ComSrvError(result['Error'])

    async def drop_all(self):
        result = await self.get({
            'DropAll': None
        })
        if 'Error' in result:
            raise ComSrvError(result['Error'])

    async def shutdown(self):
        result = await self.get({
            'Shutdown': None
        })
        if 'Error' in result:
            raise ComSrvError(result['Error'])

    async def list_instruments(self):
        result = await self.get({
            'ListInstruments': None
        })
        if 'Error' in result:
            raise ComSrvError(result['Error'])
        return result['Instruments']


from .modbus import ModBusDevice
from .bytestream import ByteStreamPipe
from .can import CanBus
from .scpi import ScpiPipe, SerialScpiPipe
from .sigrok import SigrokDevice
from .hid import HidDevice, enumerate_hid_devices
