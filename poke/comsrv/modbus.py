from typing import List

from . import get, ComSrvError, BasePipe


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
        ComSrvError.check_raise(result)

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
        ComSrvError.check_raise(result)

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
        ComSrvError.check_raise(result)
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
        ComSrvError.check_raise(result)
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
        ComSrvError.check_raise(result)
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
        ComSrvError.check_raise(result)
        return result['ModBus']['Number']

    async def send_custom_command(self, code: int, data: bytes):
        """
        Send a custom command with the given function code

        :param code: The function code, typically 0x44
        :param data: The data to be sent
        :return: A tuple of (return function code, data)
        """
        assert 0 < code < 255

        result = await get(self._url, {
            'ModBus': {
                'addr': self._addr,
                'task': {'CustomCommand': {
                    'code': code,
                    'data': list(data)
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)
        ret = result['ModBus']['Custom']
        return ret['code'], ret['data']
