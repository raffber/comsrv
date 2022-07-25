from dataclasses import dataclass
from typing import List, Optional

from . import Address, BasePipe, ComSrvError, Rpc, duration_to_json


@dataclass
class HidDeviceInfo:
    vid: int
    pid: int
    manufacturer: Optional[str] = None
    product: Optional[str] = None
    serial_number: Optional[str] = None


class HidAddress(Address):
    def __init__(self, pid: int, vid: int) -> None:
        super().__init__()
        self.pid = pid
        self.vid = vid

    def to_json(self):
        return {"pid": self.pid, "vid": self.vid}

    @property
    def enum_name(self):
        return "Hid"


class HidInstrument(object):
    def __init__(self, address: HidAddress) -> None:
        self._address = address

    def to_json(self):
        return {"address": {"pid": self._address.pid, "vid": self._address.vid}}

    @property
    def address(self):
        return self._address


class HidDevice(BasePipe):
    def __init__(self, vid, pid, rpc: Optional[Rpc] = None):
        self._instrument = HidInstrument(vid, pid)
        super().__init__(address=self._instrument.address, rpc=rpc)

    async def request(self, request):
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
        return result["Hid"]

    async def get_info(self) -> HidDeviceInfo:
        result = await self.request("GetInfo")
        dev = result["Hid"]["Info"]
        return HidDeviceInfo(
            vid=dev["idn"]["vid"],
            pid=dev["idn"]["pid"],
            manufacturer=dev.get("manufacturer"),
            product=dev.get("product"),
            serial_number=dev.get("serial_number"),
        )

    async def write(self, data):
        result = await self.request({"Write": {"data": list(data)}})
        ComSrvError.check_raise(result)

    async def read(self, timeout=0.1):
        result = await self.get({"Read": {"timeout": duration_to_json(timeout)}})
        ComSrvError.check_raise(result)
        return bytes(result["Hid"]["Data"])


async def enumerate_hid_devices(rpc=None, timeout=1.0) -> List[HidDeviceInfo]:
    if rpc is None:
        rpc = Rpc.make_default()
    result = await rpc.get({"ListHidDevices": None}, timeout)
    if "Error" in result:
        raise ComSrvError(result["Error"]["Hid"])
    devices = result["Hid"]["List"]
    ret = []
    for dev in devices:
        x = HidDeviceInfo(
            vid=dev["idn"]["vid"],
            pid=dev["idn"]["pid"],
            manufacturer=dev.get("manufacturer"),
            product=dev.get("product"),
            serial_number=dev.get("serial_number"),
        )
        ret.append(x)
    return ret
