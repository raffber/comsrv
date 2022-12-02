import asyncio
import struct
from typing import List, Union, Iterable, Optional

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

    def __call__(self, msg):
        if not isinstance(msg, DdpMessage):
            return None
        ok = msg.src == self._src and msg.dst == self._dst
        if not ok:
            return None
        return msg.data


CONTROLLER_NODEID = 0x7E
BROADCAST_ADDR = 0x7F


class GctCanBus(object):
    def __init__(self, bus: CanBus, ddp_version=1):
        self._bus = bus
        self._version = ddp_version

    @property
    def bus(self):
        return self._bus

    async def sysctrl_request(
        self, src, cmd, data: Optional[bytes] = None, dst=BROADCAST_ADDR
    ) -> bytes:
        """
        Read back sysctrl, optionally writing data to the endpoint.

        :param cmd: The command to send the data to
        :param data: The data to be sent to the command. If None, no data is sent and the data is only read back.
        :return: The data currently set at the specific command.
        """

        def flt(msg: SysCtrlMessage):
            if not isinstance(msg, SysCtrlMessage):
                return None
            if msg.src != self.dst:
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

        with self.bus.gct().map(flt) as listener:
            await self.bus.send(req)
            msg = await listener.next(timeout=0.5)
        assert isinstance(msg, SysCtrlMessage)
        return msg.data

    async def fetch_readings(
        self,
        src: int,
        group_idx: int,
        idx: Union[int, Iterable[int]],
        dst=BROADCAST_ADDR,
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
        if not is_iterable(idx):
            idx = [idx]
        else:
            idx = list(idx)

        req = MonitoringRequest()
        req.src = CONTROLLER_NODEID
        req.dst = dst
        req.group_idx = group_idx
        nodeid = src
        for x in idx:
            req.readings |= 1 << x

        def flt(msg: GctMessage):
            if not isinstance(msg, MonitoringData):
                return None
            if msg.src != nodeid:
                return None
            if msg.group_idx == group_idx and msg.reading_idx in idx:
                return msg

        ret = []
        with self.bus.gct().map(flt) as listener:
            await self.bus.send(req)
            # TODO: this not really correct, improve
            for _ in range(len(idx)):
                msg = await listener.next(timeout=0.5)
                ret.append(msg)
        ret = list(sorted(ret, key=lambda x: x.reading_idx))
        return ret

    async def read_single(self, group_idx: int, idx: int) -> MonitoringData:
        return (await self.reading_request(group_idx, [idx]))[0]

    async def read_single_and_decode(self, group_idx: int, idx: int, format: str):
        data = await self.reading_request_single(group_idx, idx)
        return struct.unpack(format, data.data)

    async def ddp(self, src_addr, dst_addr, data, want_response=False):
        # prepare ddp message
        msg = DdpMessage(version=self._version)
        data = bytearray(data)
        msg.data = bytes(data)
        msg.src = src_addr
        msg.dst = dst_addr

        # send data
        flt = DdpFilter(src=dst_addr, dst=src_addr)
        with self._bus.gct().map(flt) as listener:
            await self._bus.send(msg)
            if not want_response:
                return

            reply = await asyncio.wait_for(listener.next(), self.timeout)
            return reply


def is_iterable(arg):
    """
    Return True if the argument is an iterable. However strings are not considered an iterable
    since they usually used as primitives.
    """
    return isinstance(arg, Iterable) and not isinstance(arg, str)
