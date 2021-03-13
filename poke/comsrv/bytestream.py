from typing import Union

from poke.comsrv import get_default_http_url, get, ComSrvException


class ByteStreamPipe(object):
    def __init__(self, addr, url=None):
        if url is None:
            url = get_default_http_url()
        self._url = url
        self._addr = addr

    @property
    def url(self):
        return self._url

    async def write(self, data: bytes):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'Write': list(data)}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def read_all(self) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': 'ReadAll'
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_to_term(self, term: int, timeout: float) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadToTerm': {
                'term': term,
                'timeout_ms': int(timeout * 1e3)
            }}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_exact(self, count: int, timeout: float) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadExact': {
                'count': count,
                'timeout_ms': int(timeout * 1e3)
            }}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_upto(self, count: int) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadUpTo': count}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def cobs_write(self, data):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'CobsWrite': list(data)}
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def cobs_read(self, timeout):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'CobsQuery': int(timeout * 1e3)
            }
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def cobs_query(self, data, timeout):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'CobsQuery': {
                    'data': list(data),
                    'timeout_ms': int(timeout * 1e3)
                }
            }
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        data = bytes(result['Bytes']['Data'])
        return data

    async def write_line(self, line: str, term: Union[int, str] = '\n'):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'WriteLine': {
                    'line': line,
                    'term': term
                }
            }
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])

    async def read_line(self, timeout, term: Union[int, str] = '\n'):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'ReadLine': {
                    'term': term,
                    'timeout_ms': int(timeout * 1e3)
                }
            }
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['Bytes']['String']

    async def query_line(self, line: str, timeout, term: Union[int, str] = '\n'):
        if isinstance(term, str):
            assert len(term) == 1
            term = ord(term)
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'QueryLine': {
                    'line': line,
                    'term': term,
                    'timeout_ms': int(timeout * 1e3)
                }
            }
        }})
        if 'Error' in result:
            raise ComSrvException(result['Error'])
        return result['Bytes']['String']
