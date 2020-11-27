from typing import List

from . import get_default_http_url, get, ComSrvException


class ModBusDevice(object):
    def __init__(self, addr, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url
        self._addr = addr

    async def write_registers(self, addr: int, data: List[int]):
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'WriteRegister': {
                    'addr': addr,
                    'data': data
                }},
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def read_holding(self, addr: int, count: int = 1):
        assert count > 0
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadHolding': {
                    'addr': addr,
                    'cnt': count
                }},
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['ModBus']['Number']
