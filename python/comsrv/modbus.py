import enum
from typing import List, Optional

from pywsrpc.client import Client
from . import ComSrvError, BasePipe


class ModBusProtocol(enum.Enum):
    RTU = "Rtu"
    TCP = "Tcp"


# TODO: getters and setters


class ModBusDevice(object):
    def __init__(
        self,
        bs_pipe,
        protocol: ModBusProtocol,
        station_address: int = 1,
        timeout: float = 1.0,
    ):
        self._station_address = station_address
        self._bs_pipe = bs_pipe
        self._timeout = timeout
        self._protocol = protocol

    @property
    def protocol(self) -> ModBusProtocol:
        return self._protocol

    @property
    def station_address(self) -> int:
        return self._station_address

    @station_address.setter
    def station_address(self, value):
        self._station_address = int(value)

    async def request(self, request):
        result = await self.get(
            {
                "ModBus": {
                    "addr": self._instrument,
                    "task": request,
                    "lock": self._lock,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result

    async def write_registers(self, addr: int, data: List[int]):
        result = await self.request(
            {
                "WriteRegisters": {
                    "addr": addr,
                    "values": data,
                }
            }
        )
        ComSrvError.check_raise(result)

    async def write_coils(self, addr: int, data: List[bool]):
        result = await self.request(
            {
                "WriteCoils": {
                    "addr": addr,
                    "values": data,
                }
            },
        )
        ComSrvError.check_raise(result)

    async def read_holding(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.request(
            {
                "ReadHolding": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        ComSrvError.check_raise(result)
        return result["ModBus"]["Number"]

    async def read_coil(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.request(
            {
                "ReadHolding": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        ComSrvError.check_raise(result)
        return result["ModBus"]["Bool"]

    async def read_discrete(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.request(
            {
                "ReadDiscrete": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        ComSrvError.check_raise(result)
        return result["ModBus"]["Bool"]

    async def read_input(self, addr: int, count: int = 1):
        assert count > 0
        result = await self.request(
            {
                "ReadInput": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        ComSrvError.check_raise(result)
        return result["ModBus"]["Number"]
