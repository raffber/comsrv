from binascii import hexlify
from dataclasses import dataclass
import dataclasses
from typing import Any, Optional, Union

from . import Address, Instrument, get_default_ws_url, ComSrvError
from broadcast_wsrpc.client import Client, JsonDict, JsonType, Receiver
import re
from enum import Enum


PCAN_RE = re.compile(r"(?P<addr>.*?)::(?P<bitrate>.*?)$")


class CanAddress(Address):
    @property
    def enum_name(self) -> str:
        return "Can"


class PCanAddress(CanAddress):
    def __init__(self, address: str):
        self.address = address
        super().__init__()

    def to_json(self) -> JsonDict:
        return {"PCan": {"address": self.address}}


class SocketCanAddress(CanAddress):
    def __init__(self, interface: str):
        self._interface = interface
        super().__init__()

    def to_json(self) -> JsonDict:
        return {"SocketCan": {"interface": self._interface}}


class UsrCanetAddress(CanAddress):
    def __init__(self, host: str, port: int):
        self._host = host
        self._port = port
        super().__init__()

    def to_json(self) -> JsonDict:
        return {"UsrCanet": {"host": self._host, "port": self._port}}


class LoopbackAddress(CanAddress):
    def to_json(self) -> str:
        return "Loopback"


class CanInstrument(Instrument):
    def __init__(self, address: CanAddress, bitrate: int = 0) -> None:
        self._address = address
        self._baudrate = bitrate
        super().__init__()

    @property
    def address(self) -> Address:
        return self._address

    def to_json(self) -> JsonType:
        if isinstance(self._address, PCanAddress):
            return {
                "PCan": {"address": self._address.address, "bitrate": self._baudrate}
            }
        elif isinstance(self._address, SocketCanAddress):
            return self._address.to_json()
        elif isinstance(self._address, UsrCanetAddress):
            return self._address.to_json()
        elif isinstance(self._address, LoopbackAddress):
            return self._address.to_json()
        raise ValueError("Invalid can address")

    @property
    def enum_name(self) -> str:
        return "Can"

    @classmethod
    def parse(cls, addr: str) -> "CanInstrument":
        if addr.startswith("can::pcan::"):
            addr = addr.replace("can::pcan::", "", 1)
            match = PCAN_RE.match(addr)
            if match is None:
                raise ValueError("PCan address is invalid.")
            addr = match.group("addr")
            bitrate = int(match.group("bitrate"))
            return CanInstrument(PCanAddress(addr), bitrate=bitrate)
        elif addr.startswith("can::socket::"):
            addr = addr.replace("can::socket::", "", 1)
            return CanInstrument(SocketCanAddress(addr))
        elif addr.startswith("can::usrcanet::"):
            addr = addr.replace("can::usrcanet::", "", 1)
            splits = addr.split(":")
            if len(splits) != 2:
                raise ValueError(
                    "Expect an address of the form `can::usrcanet::<host-name>:<port>`"
                )
            host = splits[0]
            port = int(splits[1])
            return CanInstrument(UsrCanetAddress(host, port))
        elif addr.startswith("can::loopback"):
            return CanInstrument(LoopbackAddress())
        raise ValueError("Invalid address format")


@dataclass
class CanMessage:
    canid: int = 0

    def to_comsrv(self) -> JsonDict:
        raise NotImplementedError

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "CanMessage":
        if "Data" in msg:
            return DataMessage.from_comsrv(msg["Data"])
        if "Remote" in msg:
            return RemoteMessage.from_comsrv(msg["Remote"])
        raise ValueError("Invalid json object")

    def to_json(self) -> JsonDict:
        return self.to_comsrv()

    @classmethod
    def from_json(cls, msg: JsonDict) -> "CanMessage":
        return CanMessage.from_comsrv(msg)


@dataclass
class GctMessage:
    """
    A GCT message. This message may consist of several raw CAN messages on the bus.
    """

    src: int = 0

    @property
    def dst(self) -> int:
        raise NotImplementedError

    def to_comsrv(self) -> JsonDict:
        """
        Encode the message to the JSON RPC format of the comsrv.
        """
        raise NotImplementedError

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "GctMessage":
        """
        Decode the message from the comsrv RPC format
        :param msg: The JSON encoded message
        """
        if "SysCtrl" in msg:
            return SysCtrlMessage.from_comsrv(msg["SysCtrl"])
        if "MonitoringData" in msg:
            return MonitoringData.from_comsrv(msg["MonitoringData"])
        if "MonitoringRequest" in msg:
            return MonitoringRequest.from_comsrv(msg["MonitoringRequest"])
        if "Ddp" in msg:
            return DdpMessage.from_comsrv(msg["Ddp"])
        if "Heartbeat" in msg:
            return Heartbeat.from_comsrv(msg["Heartbeat"])
        raise ValueError("Invalid json object")

    def to_json(self) -> JsonDict:
        return self.to_comsrv()

    @classmethod
    def from_json(cls, msg: JsonDict) -> "GctMessage":
        return GctMessage.from_comsrv(msg)


class CanBus(object):
    def __init__(
        self, device: Union[str, CanInstrument], client: Optional[Client] = None
    ):
        if isinstance(device, str):
            device = CanInstrument.parse(device)
        if client is None:
            client = Client()
        self._client = client
        self._device = device

    async def connect(self, url: str | None = None, **kw: Any) -> "CanBus":
        """
        Connect to the server managing the CAN bus and start listening for RAW and GCT messages.

        :param url: The URL to the communication server
        :param **kw: Passed to `broadcast_wsrpc.client.Client.connect()`
        """
        if self._client.connected:
            return self
        if url is None:
            url = get_default_ws_url()
        await self._client.connect(url, **kw)
        await self.listen_gct()
        await self.listen_raw()
        return self

    async def disconnect(self) -> None:
        """Disconnect from the server"""
        await self.client.disconnect()

    async def enable_loopback(self, loopback: bool = True) -> None:
        await self.rpc({"EnableLoopback": loopback})

    async def rpc(self, task: JsonType, rx_reply: bool = True) -> JsonDict | None:
        """
        Perform an RPC to the CAN endpoint.

        :param task: The message to be sent to the CAN endpoint.
        :param rx_reply: True if a reply from the server should be awaited
        :return: The return value of the server or None if `rx_reply=False`
        """
        await self.connect()
        msg = {"Can": {"instrument": self._device.to_json(), "request": task}}
        if not rx_reply:
            await self._client.send_request(msg)
            return None
        resp = await self._client.request(msg)
        if not isinstance(resp, dict):
            raise ComSrvError("Unexpected wire format")
        ComSrvError.check_raise(resp)
        if "Can" not in resp:
            raise ComSrvError("Unexpected wire format")
        return resp["Can"]

    async def send(self, msg: CanMessage | GctMessage, rx_reply: bool = True) -> None:
        """
        Send a CAN message. The message must be either of type `comsrv.can.CanMessage`
        or `comsrv.can.GctMessage`.
        Awaiting the reply from the server is not necessary and setting `rx_reply=False`
        will disable this.

        :param msg: The message to send to the bus
        :param rx_reply: Wait for an acknowledgment of the server
        """
        if isinstance(msg, CanMessage):
            task = {"TxRaw": msg.to_comsrv()}
        elif isinstance(msg, GctMessage):
            task = {"TxGct": msg.to_comsrv()}
        else:
            raise ComSrvError("Invalid message type.")
        await self.rpc(task, rx_reply=rx_reply)

    async def listen_raw(self) -> None:
        """Start listening to raw CAN messages on the bus"""
        await self.rpc({"ListenRaw": True})

    async def listen_gct(self) -> None:
        """Start listening to GCT CAN messages on the bus"""
        await self.rpc({"ListenGct": True})

    def raw(self) -> Receiver["CanMessage"]:
        """Return a listener for raw CAN messages"""
        return self._client.listen(raw_filter)

    def gct(self) -> Receiver["GctMessage"]:
        """Return a listener for GCT CAN messages"""
        return self._client.listen(gct_filter)

    @property
    def client(self) -> Client:
        """Return the underlying RPC client"""
        return self._client


class DataMessage(CanMessage):
    """
    A single data message on the CAN bus.

    Fields are the following:
        data: bytes()
        canid: int
        extid: bool
    """

    data: bytes = b""
    canid: int = 0
    extid: bool = False

    def clone(self) -> "DataMessage":
        return dataclasses.replace(self)

    def to_comsrv(self) -> JsonDict:
        return {
            "Data": {"id": self.canid, "ext_id": self.extid, "data": list(self.data)}
        }

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "DataMessage":
        ret = cls()
        ret.canid = msg["id"]
        ret.extid = msg["ext_id"]
        ret.data = msg["data"]
        return ret

    def __repr__(self) -> str:
        return "<comsrv.can.DataMessage canid={0:x} ext_id={1} data={2}>".format(
            self.canid,
            self.extid,
            hexlify(self.data).decode(),
        )


class RemoteMessage(CanMessage):
    """
    A remote frame on the can bus

    Fields are the following:
        dlc: int
        canid: int
        extid: bool
    """

    dlc: int = 0
    canid: int = 0
    extid: bool = False

    def clone(self) -> "RemoteMessage":
        return dataclasses.replace(self)

    def to_comsrv(self) -> JsonDict:
        return {"Remote": {"id": self.canid, "ext_id": self.extid, "dlc": self.dlc}}

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "RemoteMessage":
        ret = cls()
        ret.canid = msg["id"]
        ret.extid = msg["ext_id"]
        ret.dlc = msg["dlc"]
        return ret


GCTCAN_BROADCAST_ADDRESS = 0x7F


class SysCtrlType(Enum):
    EMPTY = "None"
    VALUE = "Value"
    QUERY = "Query"


@dataclass
class SysCtrlMessage(GctMessage):
    dst: int = 0
    cmd: int = 0
    tp: SysCtrlType = SysCtrlType.EMPTY
    data: bytes = b""

    def to_comsrv(self) -> JsonDict:
        return {
            "SysCtrl": {
                "src": self.src,
                "dst": self.dst,
                "cmd": self.cmd,
                "tp": self.tp.value,
                "data": list(self.data),
            }
        }

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "SysCtrlMessage":
        ret = cls()
        ret.dst = msg["dst"]
        ret.src = msg["src"]
        ret.cmd = msg["cmd"]
        tp = msg["tp"]
        if tp == "None":
            ret.tp = SysCtrlType.EMPTY
        elif tp == "Value":
            ret.tp = SysCtrlType.VALUE
        elif tp == "Query":
            ret.tp = SysCtrlType.QUERY
        else:
            raise ValueError("No such SysCtrl type")
        ret.data = bytes(msg["data"])
        return ret


@dataclass
class MonitoringData(GctMessage):
    group_idx: int = 0
    reading_idx: int = 0
    data: bytes = b""

    @property
    def dst(self) -> int:
        return GCTCAN_BROADCAST_ADDRESS

    def to_comsrv(self) -> JsonDict:
        return {
            "MonitoringData": {
                "src": self.src,
                "group_idx": self.group_idx,
                "reading_idx": self.reading_idx,
                "data": list(self.data),
            }
        }

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "MonitoringData":
        ret = cls()
        ret.src = msg["src"]
        ret.group_idx = msg["group_idx"]
        ret.reading_idx = msg["reading_idx"]
        ret.data = bytes(msg["data"])
        return ret

    def __repr__(self) -> str:
        return f"<comsrv.can.MonitoringData src={self.src:x} group={self.group_idx} readings={self.reading_idx:x} data={hexlify(self.data).decode()}>"


@dataclass
class MonitoringRequest(GctMessage):
    dst: int = 0
    group_idx: int = 0
    readings: int = 0

    def to_comsrv(self) -> JsonDict:
        return {
            "MonitoringRequest": {
                "src": self.src,
                "dst": self.dst,
                "group_idx": self.group_idx,
                "readings": self.readings,
            }
        }

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "MonitoringRequest":
        ret = cls()
        ret.src = msg["src"]
        ret.dst = msg["dst"]
        ret.group_idx = msg["group_idx"]
        ret.readings = msg["readings"]
        return ret

    def __repr__(self) -> str:
        return "<comsrv.can.MonitoringData dst={:x} group={} readings={}>".format(
            self.dst, self.group_idx, self.readings
        )


@dataclass
class DdpMessage(GctMessage):
    dst: int = 0
    data: bytes = b""
    version: int = 2

    def to_comsrv(self) -> JsonDict:
        return {
            "Ddp": {
                "version": self.version,
                "src": self.src,
                "dst": self.dst,
                "data": list(self.data),
            }
        }

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "DdpMessage":
        ret = cls()
        ret.src = msg["src"]
        ret.dst = msg["dst"]
        ret.data = bytes(msg["data"])
        ret.version = msg.get("version", 1)
        return ret

    def __repr__(self) -> str:
        return "<comsrv.can.DdpMessage v={} src={:x} dst={:x} data={}>".format(
            self.version,
            self.src,
            self.dst,
            hexlify(self.data).decode(),
        )


@dataclass
class Heartbeat(GctMessage):
    product_id: int = 0

    def to_comsrv(self) -> JsonDict:
        return {
            "Heartbeat": {
                "src": self.src,
                "product_id": self.product_id,
            }
        }

    @property
    def dst(self) -> int:
        return GCTCAN_BROADCAST_ADDRESS

    @classmethod
    def from_comsrv(cls, msg: JsonDict) -> "Heartbeat":
        ret = cls()
        ret.src = msg["src"]
        ret.product_id = msg["product_id"]
        return ret


def gct_filter(msg: JsonDict) -> GctMessage | None:
    if "Notify" not in msg:
        return None
    if "Can" not in msg["Notify"]:
        return None
    can = msg["Notify"]["Can"]["response"]
    if "Gct" in can:
        msg = can["Gct"]
        return GctMessage.from_comsrv(msg)
    return None


def raw_filter(msg: JsonDict) -> CanMessage | None:
    if "Notify" not in msg:
        return None
    if "Can" not in msg["Notify"]:
        return None
    can = msg["Notify"]["Can"]["response"]
    if "Raw" in can:
        msg = can["Raw"]
        return CanMessage.from_comsrv(msg)
    return None
