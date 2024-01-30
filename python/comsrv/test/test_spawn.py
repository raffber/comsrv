from comsrv import ComSrv
from comsrv.spawn import spawn
import pytest


@pytest.mark.asyncio
async def test_spawn():
    rpc = await spawn()
    comsrv = ComSrv(rpc)
    version = await comsrv.get_version()
    assert version == (2, 2, 0)
