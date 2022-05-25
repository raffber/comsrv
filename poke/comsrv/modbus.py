from typing import List, Optional

from pywsrpc.client import Client
from . import ComSrvError, BasePipe


class ModBusDevice(BasePipe):
    def __init__(self, addr, station_address: int = 0, rpc: Optional[Client] = None):
        super().__init__(addr, rpc=rpc)
        self._station_address = station_address

    @property
    def station_address(self) -> int:
        return self._station_address

    @station_address.setter
    def station_address(self, value):
        self._station_address = int(value)

    async def write_registers(self, addr: int, data: List[int]):
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'WriteRegister': {
                    'addr': addr,
                    'data': data,
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)

    async def write_coils(self, addr: int, data: List[bool]):
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'WriteCoil': {
                    'addr': addr,
                    'data': data,
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)

    async def read_holding(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadHolding': {
                    'addr': addr,
                    'cnt': count,
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)
        return result['ModBus']['Number']

    async def read_coil(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadHolding': {
                    'addr': addr,
                    'cnt': count,
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)
        return result['ModBus']['Bool']

    async def read_discrete(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadDiscrete': {
                    'addr': addr,
                    'cnt': count,
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)
        return result['ModBus']['Bool']

    async def read_input(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'ReadInput': {
                    'addr': addr,
                    'cnt': count,
                    'slave_id': self._station_address,
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

        result = await self.get({
            'ModBus': {
                'addr': self._addr,
                'task': {'CustomCommand': {
                    'code': code,
                    'data': list(data),
                    'slave_id': self._station_address,
                }},
                'lock': self._lock,
            }
        })
        ComSrvError.check_raise(result)
        ret = result['ModBus']['Custom']
        return ret['code'], ret['data']
