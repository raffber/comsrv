from dataclasses import dataclass
from typing import List, Optional

from poke.comsrv import BasePipe, get_default_http_url, get, ComSrvError


@dataclass
class HidDeviceInfo:
    vid: int
    pid: int
    manufacturer: Optional[str] = None
    product: Optional[str] = None
    serial_number: Optional[str] = None


class HidDevice(BasePipe):
    def __init__(self, vid, pid, url=None):
        super().__init__('can::{}::{}'.format(vid, pid), url=url)

    async def get_info(self) -> HidDeviceInfo:
        result = await self.get({'Hid': {
            'addr': self.addr,
            'task': 'GetInfo'}})
        ComSrvError.check_raise(result)
        dev = result['Hid']['Info']
        return HidDeviceInfo(vid=dev['idn']['vid'], pid=dev['idn']['pid'], manufacturer=dev.get('manufacturer'),
                             product=dev.get('product'), serial_number=dev.get('serial_number'))

    async def write(self, data):
        result = await self.get({'Hid': {
            'addr': self.addr,
            'task': {'Write': {'data': data}}
        }})
        ComSrvError.check_raise(result)

    async def read(self, timeout_ms: int = 100):
        result = await self.get({'Hid': {
            'addr': self.addr,
            'task': {'Read': {'timeout_ms': timeout_ms}}
        }})
        ComSrvError.check_raise(result)
        return bytes(result['Hid']['Data'])


async def enumerate_hid_devices(url=None) -> List[HidDeviceInfo]:
    if url is None:
        url = get_default_http_url()
    result = await get(url, {'ListHidDevices': None})
    if 'Error' in result:
        raise ComSrvError(result['Error']['Hid'])
    devices = result['Hid']['List']
    ret = []
    for dev in devices:
        x = HidDeviceInfo(vid=dev['idn']['vid'], pid=dev['idn']['pid'], manufacturer=dev.get('manufacturer'),
                          product=dev.get('product'), serial_number=dev.get('serial_number'))
        ret.append(x)
    return ret
