import enum
from typing import List, Optional, Tuple, Union

from comsrv.bytestream import (
    ByteStreamInstrument,
    ByteStreamPipe,
    SerialAddress,
    SerialInstrument,
    SerialPortConfig,
    TcpAddress,
    TcpInstrument,
)

from . import ComSrvError, Rpc, duration_to_json
import re
from broadcast_wsrpc import JsonType


class ModBusProtocol(enum.Enum):
    RTU = "Rtu"
    TCP = "Tcp"


MODBUS_SERIAL_RE = re.compile(
    r"modbus::(?P<protocol>(rtu)|(tcp))::(?P<port>.*?)::(?P<baudrate>\d+)::(?P<config>[78][ENO][12])(::(?P<station_address>\d+))?"
)

MODBUS_TCP_RE = re.compile(
    r"modbus::(?P<protocol>(rtu)|(tcp))::(?P<host>.*?):(?P<port>\d+)(::(?P<station_address>\d+))?"
)


def parse_serial_modbus(
    match: re.Match[str], rpc: Rpc | None
) -> Tuple[ByteStreamPipe, int, ModBusProtocol]:
    protocol = match.group("protocol")
    if protocol == "rtu":
        ret_protocol = ModBusProtocol.RTU
    elif protocol == "tcp":
        ret_protocol = ModBusProtocol.TCP
    else:
        raise AssertionError
    baudrate = int(match.group("baudrate"))
    config = match.group("config")
    port = match.group("port")
    station_address = match.group("station_address")
    if station_address is None:
        station_address = 255
    else:
        station_address = int(station_address)
    address = SerialAddress(port)
    port_config = SerialPortConfig(baudrate=baudrate, config=config)
    instrument = SerialInstrument(address, port_config)
    bs_pipe = ByteStreamPipe(instrument, rpc=rpc)
    return bs_pipe, station_address, ret_protocol


def parse_tcp_modbus(
    match: re.Match[str], rpc: Rpc | None
) -> Tuple[ByteStreamPipe, int, ModBusProtocol]:
    protocol = match.group("protocol")
    if protocol == "rtu":
        ret_protocol = ModBusProtocol.RTU
    elif protocol == "tcp":
        ret_protocol = ModBusProtocol.TCP
    else:
        raise AssertionError
    host = match.group("host")
    port = int(match.group("port"))
    if port < 1 or port > 65535:
        raise ValueError("Port needs to be in range (1, 65535)")
    station_address = match.group("station_address")
    if station_address is None:
        station_address = 255
    else:
        station_address = int(station_address)
    address = TcpAddress(host, port)
    instrument = TcpInstrument(address)
    bs_pipe = ByteStreamPipe(instrument, rpc=rpc)
    return bs_pipe, station_address, ret_protocol


def parse_modbus_address(
    address: str, rpc: Rpc | None = None
) -> Tuple[ByteStreamPipe, int, ModBusProtocol]:
    m = MODBUS_SERIAL_RE.match(address)
    if m is not None:
        return parse_serial_modbus(m, rpc)
    m = MODBUS_TCP_RE.match(address)
    if m is not None:
        return parse_tcp_modbus(m, rpc)
    raise ValueError("Could not parse address: `{}`".format(address))


class ModBusDevice(object):
    """
    Interface to a device talking ModBus.

    The ModBus protocol runs on an underlying bi-directional bytestream,
    hence it owns a `ByteStreamPipe`.
    Instead of a `ByteStreamPipe` a resource descriptor string may be provided.
    In this case, the optional `rpc = None` parameter is used to construct the
    `ByteStreamPipe`.
    """

    def __init__(
        self,
        bs_pipe: Union[str, ByteStreamPipe],
        rpc: Rpc | None = None,
        protocol: Optional[ModBusProtocol] = None,
        station_address: int = 1,
        timeout: float = 1.0,
    ) -> None:
        if isinstance(bs_pipe, str):
            if (
                bs_pipe.startswith("tcp::")
                or bs_pipe.startswith("serial::")
                or bs_pipe.startswith("ftdi::")
            ):
                bs_pipe = ByteStreamPipe(bs_pipe, rpc=rpc)
            else:
                bs_pipe, station_address, protocol = parse_modbus_address(
                    bs_pipe, rpc=rpc
                )
        if protocol is None:
            protocol = ModBusProtocol.RTU
        self._station_address = station_address
        self._bs_pipe = bs_pipe
        self._timeout = timeout
        self._protocol = protocol

    @property
    def bytestream_pipe(self) -> ByteStreamPipe:
        return self._bs_pipe

    @property
    def instrument(self) -> ByteStreamInstrument:
        return self._bs_pipe.instrument

    @property
    def protocol(self) -> ModBusProtocol:
        return self._protocol

    @protocol.setter
    def protocol(self, value: ModBusProtocol) -> None:
        self._protocol = value

    @property
    def timeout(self) -> float:
        return self._timeout

    @timeout.setter
    def timeout(self, value: float) -> None:
        self._timeout = value

    @property
    def station_address(self) -> int:
        return self._station_address

    @station_address.setter
    def station_address(self, value: int) -> None:
        self._station_address = value

    async def request(self, request: JsonType) -> JsonType:
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
        return result["ModBus"]

    async def write_registers(self, addr: int, data: List[int]) -> None:
        await self.request(
            {
                "WriteRegisters": {
                    "addr": addr,
                    "values": data,
                }
            }
        )

    async def write_coils(self, addr: int, data: List[bool]) -> None:
        await self.request(
            {
                "WriteCoils": {
                    "addr": addr,
                    "values": data,
                }
            },
        )

    async def read_holding(self, addr: int, count: int = 1) -> List[int]:
        assert count > 0
        result = await self.request(
            {
                "ReadHolding": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        assert isinstance(result, dict)
        ret = result["Number"]
        assert isinstance(ret, list)
        return ret  # type: ignore

    async def read_coil(self, addr: int, count: int = 1) -> List[bool]:
        assert count > 0
        result = await self.request(
            {
                "ReadHolding": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        assert isinstance(result, dict)
        ret = result["Bool"]
        assert isinstance(ret, list)
        return ret  # type: ignore

    async def read_discrete(self, addr: int, count: int = 1) -> List[bool]:
        assert count > 0
        result = await self.request(
            {
                "ReadDiscrete": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        assert isinstance(result, dict)
        ret = result["Bool"]
        assert isinstance(ret, list)
        return ret  # type: ignore

    async def read_input(self, addr: int, count: int = 1) -> List[int]:
        assert count > 0
        result = await self.request(
            {
                "ReadInput": {
                    "addr": addr,
                    "cnt": count,
                }
            },
        )
        assert isinstance(result, dict)
        ret = result["Number"]
        assert isinstance(ret, list)
        return ret  # type: ignore

    async def ddp(
        self,
        sub_cmd: int,
        ddp_cmd: int,
        data: bytes,
        response: bool = True,
    ) -> bytes:
        result = await self.request(
            {
                "Ddp": {
                    "sub_cmd": sub_cmd,
                    "ddp_cmd": ddp_cmd,
                    "response": response,
                    "data": list(data),
                }
            },
        )
        assert isinstance(result, dict)
        d = result["Data"]
        return bytes(d)  # type: ignore
