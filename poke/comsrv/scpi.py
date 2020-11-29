import base64

from poke.comsrv import get_default_http_url, get, ComSrvException
from poke.scpi import Pipe as ScpiPipeBase


class ScpiPipe(ScpiPipeBase):
    def __init__(self, addr: str, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url
        self._addr = addr

    @property
    def url(self):
        return self._url

    async def query(self, msg: str) -> str:
        result = await get(self._url, {'Scpi': {
            'addr': self._addr,
            'task': {'QueryString': msg},
            'options': {'Default': None}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['Scpi']['String']

    async def write(self, msg: str):
        result = await get(self._url, {'Scpi': {
            'addr': self._addr,
            'task': {'Write': msg},
            'options': {'Default': None}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def query_binary(self, msg: str) -> bytes:
        result = await get(self._url, {
            'Scpi': {
                'addr': self._addr,
                'task': {'QueryBinary': msg},
                'options': {'Default': None}
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = result['Scpi']['Binary']['data']
        return base64.b64decode(data)

    async def read_raw(self) -> bytes:
        result = await get(self._url, {
            'Scpi': {
                'addr': self._addr,
                'task': 'ReadRaw',
                'options': {'Default': None}
            }
        })
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = result['Scpi']['Binary']['data']
        return base64.b64decode(data)
