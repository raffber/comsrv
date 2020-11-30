import base64

from poke.comsrv import get_default_http_url, get, ComSrvException, SerialPipe
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


class SerialScpiPipe(ScpiPipeBase):
    def __init__(self, addr, url=None, term='\n', timeout=1.0):
        super().__init__()
        self._inner = SerialPipe(addr, url=url)
        self._timeout = timeout
        self._term = term

    @property
    def term(self):
        return self._term

    @term.setter
    def term(self, value):
        self._term = value

    @property
    def timeout(self):
        return self._timeout

    @timeout.setter
    def timeout(self, value):
        self._timeout = value

    async def query(self, msg: str) -> str:
        return await self._inner.query_line(msg, self._timeout, term=self._term)

    async def write(self, msg: str):
        return await self._inner.write_line(msg, term=self._term)

    async def query_binary(self, msg: str) -> bytes:
        raise NotImplementedError

    async def read_raw(self) -> bytes:
        return await self._inner.read_line(self._timeout, term=self._term)