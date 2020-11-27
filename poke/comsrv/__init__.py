"""
This modules implements the API of the `comsrv` utility to connect
to instruments.
"""

import json

from aiohttp import ClientSession


class ComSrvException(Exception):
    pass


_default_port = 5903
_default_host = '127.0.0.1'


def setup_default_comsrv(host=None, port=None):
    global _default_host
    global _default_port
    if host is None:
        host = _default_host
    if port is None:
        port = _default_port
    _default_host = host
    _default_port = port


def _get_default_url():
    return 'http://{}:{}'.format(_default_host, _default_port)


async def get(addr, data):
    data = json.dumps(data).encode()
    async with ClientSession() as session:
        async with session.get(addr, data=data) as resp:
            if resp.status != 200:
                raise ComSrvException()
            data = json.loads(await resp.text())
            return data


from .scpi import ScpiPipe
from .modbus import ModBusDevice
from .serial import SerialPipe
