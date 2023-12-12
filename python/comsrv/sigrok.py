from typing import List, Optional

import numpy as np

from . import Rpc, ComSrvError


TIMEOUT = 5.0


class SigrokDevice(object):
    def __init__(self, addr, desc=None, rpc: None | Rpc = None):
        if rpc is None:
            rpc = Rpc.make_default()
        self._rpc = rpc
        self._addr = addr
        self._desc = desc

    @property
    def description(self):
        return self._desc

    @property
    def address(self):
        return self._addr

    async def read(
        self,
        channels: Optional[List[str]] = None,
        samplerate=48e6,
        num_samples=None,
        time=None,
    ):
        if time is not None and num_samples is not None:
            raise ValueError("Specifiy only one of time or num_samples")
        if time is not None:
            acquire = {"Time": float(time)}
        elif num_samples is not None:
            acquire = {"Samples": int(num_samples)}
        else:
            raise ValueError("Neither time nor num_samples is given")
        if channels is None:
            channels = []
        request = {
            "Sigrok": {
                "instrument": {"address": self._addr},
                "request": {
                    "channels": channels,
                    "sample_rate": int(samplerate),
                    "acquire": acquire,
                },
            }
        }
        data = await self._rpc.get(request, TIMEOUT)
        ComSrvError.check_raise(data)
        data = data["Sigrok"]["Data"]
        tsample = data["tsample"]
        length = data["length"]
        t = np.arange(0, length) * tsample
        ret = {}
        for (k, v) in data["channels"].items():
            base = np.array(v, dtype=np.uint8)
            ret[k] = np.unpackbits(base, count=length, bitorder="little")
        return t, ret


async def list_devices(rpc: Optional[Rpc] = None) -> List[SigrokDevice]:
    if rpc is None:
        rpc = Rpc.make_default()
    ret = await rpc.get({"ListSigrokDevices": None}, TIMEOUT)
    ComSrvError.check_raise(ret)
    devices = ret["Sigrok"]["Devices"]
    ret = []
    for dev in devices:
        ret.append(SigrokDevice(dev["addr"], desc=dev["desc"], rpc=rpc))
    return ret
