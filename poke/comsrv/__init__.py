"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

import json

from aiohttp import ClientSession

from pywsrpc.client import Client


class ComSrvException(Exception):
    pass


_default_ws_port = 5902
_default_http_port = 5903
_default_host = '127.0.0.1'


def setup_default_comsrv(host=None, http_port=None, ws_port=None):
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


async def get(addr, data):
    data = json.dumps(data).encode()
    async with ClientSession() as session:
        async with session.get(addr, data=data) as resp:
            data = json.loads(await resp.text())
            if resp.status != 200:
                raise ComSrvException(data)
            return data


async def connect_client(url=None):
    if url is None:
        url = get_default_ws_url()
    client = Client()
    await client.connect(url)
    return client


from .modbus import ModBusDevice
from .serial import SerialPipe
from .can import CanBus
from .scpi import ScpiPipe
