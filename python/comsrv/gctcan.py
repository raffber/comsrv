import asyncio
import struct
from typing import Any, List, Set, Union, Iterable, Optional
from broadcast_wsrpc.client import Receiver  # type: ignore

from .can import (
    DdpMessage,
    SysCtrlMessage,
    SysCtrlType,
    MonitoringRequest,
    GctMessage,
    MonitoringData,
    CanBus,
)


class DdpFilter(object):
    def __init__(self, src: int, dst: int):
        self._src = src
        self._dst = dst

    def __call__(self, msg: GctMessage) -> bytes | None:
        if not isinstance(msg, DdpMessage):
            return None
        ok = msg.src == self._src and msg.dst == self._dst
        if not ok:
            return None
        return msg.data


CONTROLLER_NODEID = 0x7E
BROADCAST_ADDR = 0x7F
DEFAULT_DDP_TIMEOUT = 0.1


class GctCanBus(object):
    def __init__(self, bus: CanBus, ddp_version: int = 2) -> None:
        self._bus = bus
        self._version = ddp_version

    @property
    def can_bus(self) -> CanBus:
        return self._bus

    async def connect(self) -> "GctCanBus":
        await self._bus.connect()
        return self

    async def sysctrl_request(
        self,
        src: int,
        cmd: int,
        data: bytes | None = None,
        dst: int = BROADCAST_ADDR,
    ) -> bytes:
        """
        Read back sysctrl, optionally writing data to the endpoint.

        :param cmd: The command to send the data to
        :param data: The data to be sent to the command. If None, no data is sent and the data is only read back.
        :return: The data currently set at the specific command.
        """

        def flt(msg: GctMessage) -> SysCtrlMessage | None:
            if not isinstance(msg, SysCtrlMessage):
                return None
            if msg.src != dst and dst != BROADCAST_ADDR:
                return None
            if msg.tp != SysCtrlType.VALUE:
                return None
            if msg.cmd != cmd:
                return None
            return msg

        req = SysCtrlMessage()
        req.cmd = cmd
        req.src = src
        req.dst = dst
        req.tp = SysCtrlType.QUERY
        if data is None:
            data = b""
        req.data = data

        with self.can_bus.gct().map(flt) as listener:
            await self.can_bus.send(req)
            msg = await listener.next(timeout=0.5)
        assert isinstance(msg, SysCtrlMessage)
        return msg.data

    async def fetch_readings(
        self,
        src: int,
        group_idx: int,
        idx: Union[int, Iterable[int]],
        dst: int = BROADCAST_ADDR,
    ) -> List[MonitoringData]:
        """
        Fetch readings from the device.
        In case not all requested readings are received, this function times out.

        :param src: The source nodeid
        :param dst: The destination nodeid, default to the broadcast address
        :param group_idx: The group index to fetch
        :param idx: Either one reading index or an iterable thereof
        :return: A list of MonitoringData messages, sorted by reading index
        """
        unified_idx: Set[int] = set()
        if not is_iterable(idx):
            unified_idx = {idx}  # type: ignore
        else:
            unified_idx = set(idx)  # type: ignore

        req = MonitoringRequest()
        req.src = src
        req.dst = dst
        req.group_idx = group_idx
        for x in unified_idx:
            req.readings |= 1 << x

        def flt(msg: GctMessage) -> Optional[MonitoringData]:
            if not isinstance(msg, MonitoringData):
                return None
            if msg.src != dst and dst == BROADCAST_ADDR:
                return None
            if msg.group_idx == group_idx and msg.reading_idx in unified_idx:
                return msg
            return None

        with self.can_bus.gct().map(flt) as listener:
            await self.can_bus.send(req)
            return await asyncio.wait_for(
                self._receive_readings(listener, len(unified_idx)), timeout=0.1
            )

    async def _receive_readings(
        self, listener: Receiver, num_unique: int
    ) -> List[MonitoringData]:
        ret = {}
        while True:
            msg = await listener.next()
            assert isinstance(msg, MonitoringData)
            ret[msg.reading_idx] = msg
            if len(ret) == num_unique:
                break
        return list(sorted(list(ret.values()), key=lambda x: x.reading_idx))

    async def read_single(self, src: int, group_idx: int, idx: int) -> MonitoringData:
        return (await self.fetch_readings(src, group_idx, [idx]))[0]

    async def read_single_and_decode(
        self, src: int, group_idx: int, idx: int, format: str
    ) -> tuple:
        data = await self.read_single(src, group_idx, idx)
        return struct.unpack(format, data.data)

    async def ddp(
        self,
        src_addr: int,
        dst_addr: int,
        data: bytes,
        timeout: float,
    ) -> bytes | None:
        assert len(data) > 0
        msg = DdpMessage(version=self._version)
        data = bytearray(data)
        want_response = (data[0] & 0x80) != 0
        msg.data = bytes(data)
        msg.src = src_addr
        msg.dst = dst_addr

        flt = DdpFilter(src=dst_addr, dst=src_addr)
        with self.can_bus.gct().map(flt) as listener:
            await self.can_bus.send(msg)
            if not want_response:
                return None

            reply = await asyncio.wait_for(listener.next(), timeout)
            return reply

    async def ddp_request(
        self,
        cmd: int,
        src_addr: int,
        dst_addr: int,
        data: bytes,
        timeout: float = DEFAULT_DDP_TIMEOUT,
    ) -> bytes:
        assert cmd < 256, "Command must be a byte"
        data = bytes([cmd | 0x80]) + data
        reply = await self.ddp(src_addr, dst_addr, data, timeout)
        assert reply is not None
        return reply

    async def ddp_write(
        self,
        cmd: int,
        src_addr: int,
        dst_addr: int,
        data: bytes,
        timeout: float = DEFAULT_DDP_TIMEOUT,
    ) -> None:
        data = bytes([cmd & 0x7F]) + data
        await self.ddp(src_addr, dst_addr, data, timeout=timeout)


def is_iterable(arg: Any) -> bool:
    """
    Return True if the argument is an iterable. However strings are not considered an iterable
    since they usually used as primitives.
    """
    return isinstance(arg, Iterable) and not isinstance(arg, str)
