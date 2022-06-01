from typing import List, Optional

import numpy as np

from poke.comsrv import get_default_http_url, get, ComSrvError


class SigrokDevice(object):
    def __init__(self, addr, url=None, desc=None):
        if url is None:
            url = get_default_http_url()
        self._url = url
        self._addr = addr
        self._desc = desc

    @property
    def description(self):
        return self._desc

    @property
    def address(self):
        return self._addr

    async def read(self, channels: Optional[List[str]] = None, samplerate=48e6, num_samples=None, time=None):
        if time is not None and num_samples is not None:
            raise ValueError("Specifiy only one of time or num_samples")
        if time is not None:
            acquire = {'Time': float(time)}
        elif num_samples is not None:
            acquire = {'Samples': int(num_samples)}
        else:
            raise ValueError('Neither time nor num_samples is given')
        if channels is None:
            channels = []
        request = {'Sigrok': {
            'addr': self._addr,
            'task': {
                'channels': channels,
                'sample_rate': int(samplerate),
                'acquire': acquire
            }
        }}
        data = await get(self._url, request)
        ComSrvError.check_raise(data)
        data = data['Sigrok']['Data']
        tsample = data['tsample']
        length = data['length']
        t = np.arange(0, length) * tsample
        ret = {}
        for (k, v) in data['channels'].items():
            base = np.array(v, dtype=np.uint8)
            ret[k] = np.unpackbits(base, count=length, bitorder='little')
        return t, ret


async def list_devices(url=None) -> List[SigrokDevice]:
    if url is None:
        url = get_default_http_url()
    ret = await get(url, {'ListSigrokDevices': None})
    ComSrvError.check_raise(ret)
    devices = ret['Sigrok']['Devices']
    ret = []
    for dev in devices:
        ret.append(SigrokDevice(dev['addr'], url=url, desc=dev['desc']))
    return ret
