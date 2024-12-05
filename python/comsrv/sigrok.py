from typing import List, Optional

import numpy as np
import numpy.typing as npt

from . import Rpc, ComSrvError
from broadcast_wsrpc import JsonObject


TIMEOUT = 5.0


class SigrokDevice(object):
    def __init__(
        self,
        addr: str,
        desc: str | None = None,
        rpc: None | Rpc = None,
    ) -> None:
        if rpc is None:
            rpc = Rpc.make_default()
        self._rpc = rpc
        self._addr = addr
        self._desc = desc

    @property
    def description(self) -> str | None:
        return self._desc

    @property
    def address(self) -> str:
        return self._addr

    async def read(
        self,
        channels: Optional[List[str]] = None,
        samplerate: float | int = 48e6,
        num_samples: int | None = None,
        time: float | None = None,
    ) -> tuple[npt.NDArray[np.float64], dict[str, npt.NDArray[np.float64]]]:
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
        data: JsonObject = data["Sigrok"]["Data"]  # type: ignore
        tsample = float(data["tsample"])  # type: ignore
        length = data["length"]
        assert isinstance(length, int)
        t = np.arange(0, length) * tsample
        ret = {}
        for k, v in data["channels"].items():  # type: ignore
            base = np.array(v, dtype=np.uint8)
            ret[k] = np.unpackbits(base, count=length, bitorder="little")
        return t, ret  # type: ignore


async def list_devices(rpc: Optional[Rpc] = None) -> List[SigrokDevice]:
    if rpc is None:
        rpc = Rpc.make_default()
    resp = await rpc.get({"ListSigrokDevices": None}, TIMEOUT)
    ComSrvError.check_raise(resp)
    devices: List[JsonObject] = resp["Sigrok"]["Devices"]  # type: ignore
    ret: List[SigrokDevice] = []
    for dev in devices:
        ret.append(SigrokDevice(dev["addr"], desc=dev["desc"], rpc=rpc))  # type: ignore
    return ret
