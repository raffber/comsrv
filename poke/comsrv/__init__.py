"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

import json
from typing import Union

from aiohttp import ClientSession

from pywsrpc.client import Client


class ComSrvException(Exception):
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


async def get(url, data):
    data = json.dumps(data).encode()
    async with ClientSession() as session:
        async with session.get(url, data=data) as resp:
            data = json.loads(await resp.text())
            if resp.status != 200:
                raise ComSrvException(data)
            return data


class BasePipe(object):
    def __init__(self, addr, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url
        self._addr = addr
        self._lock_time = 1.0
        self._lock = None

    @property
    def url(self):
        return self._url

    @property
    def lock_time(self):
        return self._lock_time

    @lock_time.setter
    def lock_time(self, value: float):
        self._lock_time = value

    @property
    def locked(self):
        return self._lock is not None

    async def lock(self, timeout: Union[float, None] = None):
        await self.unlock()
        lock_time = timeout or self._lock_time
        reply = await get(self._url, {'Lock': {
            'addr': self._addr,
            'timeout_ms': int(lock_time * 1000),
        }})
        self._lock = reply['Locked']['lock_id']
        return self

    async def __aenter__(self):
        return await self.lock()

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.unlock()

    async def unlock(self):
        if self._lock is None:
            return
        await get(self._url, {'Unlock': self._lock})
        self._lock = None


async def connect_client(url=None):
    if url is None:
        url = get_default_ws_url()
    client = Client()
    await client.connect(url)
    return client


class ComSrv(object):
    def __init__(self, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url

    async def drop(self, addr):
        result = await get(self._url, {
            'Drop': addr
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def drop_all(self):
        result = await get(self._url, {
            'DropAll': None
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def shutdown(self):
        result = await get(self._url, {
            'Shutdown': None
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def list_instruments(self):
        result = await get(self._url, {
            'ListInstruments': None
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['Instruments']


from .modbus import ModBusDevice
from .bytestream import ByteStreamPipe
from .can import CanBus
from .scpi import ScpiPipe, SerialScpiPipe
from .sigrok import SigrokDevice
