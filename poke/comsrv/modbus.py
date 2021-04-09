from typing import List

from . import get, ComSrvException, BasePipe


class ModBusDevice(BasePipe):
    async def write_registers(self, addr: int, data: List[int]):
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'WriteRegister': {
                    'addr': addr,
                    'data': data
                }},
                'lock': self._lock,
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def write_coils(self, addr: int, data: List[bool]):
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'WriteCoil': {
                    'addr': addr,
                    'data': data
                }},
                'lock': self._lock,
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
                'lock': self._lock,
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['ModBus']['Number']

    async def read_coil(self, addr: int, count: int = 1):
        assert count > 0
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadHolding': {
                    'addr': addr,
                    'cnt': count
                }},
                'lock': self._lock,
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['ModBus']['Bool']

    async def read_discrete(self, addr: int, count: int = 1):
        assert count > 0
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadDiscrete': {
                    'addr': addr,
                    'cnt': count
                }},
                'lock': self._lock,
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['ModBus']['Bool']

    async def read_input(self, addr: int, count: int = 1):
        assert count > 0
        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadInput': {
                    'addr': addr,
                    'cnt': count
                }},
                'lock': self._lock,
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['ModBus']['Number']
