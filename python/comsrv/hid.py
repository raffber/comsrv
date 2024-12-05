from typing import List, Optional

from . import Address, BasePipe, ComSrvError, Rpc, duration_to_json, HidDeviceInfo
from broadcast_wsrpc import JsonObject, JsonType


class HidAddress(Address):
    def __init__(self, pid: int, vid: int) -> None:
        super().__init__()
        self.pid = pid
        self.vid = vid

    def to_json(self) -> JsonObject:
        return {"pid": self.pid, "vid": self.vid}

    @property
    def enum_name(self) -> str:
        return "Hid"


class HidInstrument(object):
    def __init__(self, address: HidAddress) -> None:
        self._address = address

    def to_json(self) -> JsonObject:
        return {"address": {"pid": self._address.pid, "vid": self._address.vid}}

    @property
    def address(self) -> HidAddress:
        return self._address


class HidDevice(BasePipe):
    def __init__(self, vid: int, pid: int, rpc: Optional[Rpc] = None):
        self._instrument = HidInstrument(HidAddress(vid, pid))
        super().__init__(address=self._instrument.address, rpc=rpc)

    async def request(self, request: JsonType) -> JsonObject:
        result = await self.get(
            {
                "Hid": {
                    "instrument": self._instrument.to_json(),
                    "request": request,
                    "lock": self._lock,
                }
            }
        )
        ComSrvError.check_raise(result)
        hid = result["Hid"]
        assert isinstance(hid, dict)
        return hid

    async def get_info(self) -> HidDeviceInfo:
        result = await self.request("GetInfo")
        hid = result["Hid"]
        assert isinstance(hid, dict)
        dev = hid["Info"]
        return HidDeviceInfo(
            vid=dev["idn"]["vid"],  # type: ignore
            pid=dev["idn"]["pid"],  # type: ignore
            manufacturer=dev.get("manufacturer"),  # type: ignore
            product=dev.get("product"),  # type: ignore
            serial_number=dev.get("serial_number"),  # type: ignore
        )

    async def write(self, data: bytes) -> None:
        result = await self.request({"Write": {"data": list(data)}})
        ComSrvError.check_raise(result)

    async def read(self, timeout: float = 0.1) -> bytes:
        result = await self.get({"Read": {"timeout": duration_to_json(timeout)}})
        ComSrvError.check_raise(result)
        return bytes(result["Hid"]["Data"])  # type: ignore


async def enumerate_hid_devices(
    rpc: Rpc | None = None,
    timeout: float = 1.0,
) -> List[HidDeviceInfo]:
    if rpc is None:
        rpc = Rpc.make_default()
    result = await rpc.get({"ListHidDevices": None}, timeout)
    if "Error" in result:
        raise ComSrvError(result["Error"]["Hid"])  # type: ignore
    devices: List[JsonObject] = result["Hid"]["List"]  # type: ignore
    ret: List[HidDeviceInfo] = []
    for dev in devices:
        x = HidDeviceInfo(
            vid=dev["idn"]["vid"],  # type: ignore
            pid=dev["idn"]["pid"],  # type: ignore
            manufacturer=dev.get("manufacturer"),  # type: ignore
            product=dev.get("product"),  # type: ignore
            serial_number=dev.get("serial_number"),  # type: ignore
        )
        ret.append(x)
    return ret
