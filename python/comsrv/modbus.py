import enum
from typing import List

from . import ComSrvError, duration_to_json


class ModBusProtocol(enum.Enum):
    RTU = "Rtu"
    TCP = "Tcp"


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

    @protocol.setter
    def protocol(self, value: ModBusProtocol):
        self._protocol = value

    @property
    def timeout(self) -> float:
        return self._timeout

    @timeout.setter
    def timeout(self, value: float):
        self._timeout = value

    @property
    def station_address(self) -> int:
        return self._station_address

    @station_address.setter
    def station_address(self, value):
        self._station_address = int(value)

    async def request(self, request):
        result = await self._bs_pipe.request(
            {
                "ModBus": {
                    "timeout": duration_to_json(self._timeout),
                    "station_address": self.station_address,
                    "protocol": self.protocol.value,
                    "request": request,
                }
            }
        )
        ComSrvError.check_raise(result)
        return result["Bytes"]["ModBus"]

    async def write_registers(self, addr: int, data: List[int]):
        result = await self.request(
            {
                "WriteRegisters": {
                    "addr": addr,
                    "values": data,
                }
            }
        )

    async def write_coils(self, addr: int, data: List[bool]):
        result = await self.request(
            {
                "WriteCoils": {
                    "addr": addr,
                    "values": data,
                }
            },
        )

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
        return result["Number"]

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
        return result["Bool"]

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
        return result["Bool"]

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
        return result["Number"]

    async def ddp(
        self, sub_cmd: int, ddp_cmd: int, data: bytes, response=True
    ) -> bytes:
        result = await self.request(
            {
                "Ddp": {
                    "sub_cmd": sub_cmd,
                    "ddp_cmd": ddp_cmd,
                    "reponse": response,
                    "data": list(data),
                }
            },
        )
        return result["Data"]
