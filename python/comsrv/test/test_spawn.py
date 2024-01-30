from comsrv import ComSrv
from comsrv.spawn import start_comsrv
import pytest


@pytest.mark.asyncio
async def test_spawn():
    rpc = await start_comsrv()
    comsrv = ComSrv(rpc)
    version = await comsrv.get_version()
    assert version == (2, 2, 0)
