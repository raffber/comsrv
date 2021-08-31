from typing import Union

from poke.comsrv import get, ComSrvError, BasePipe


class ByteStreamPipe(BasePipe):
    async def write(self, data: bytes):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'Write': list(data)},
            'lock': self._lock
        }})
        ComSrvError.check_raise(result)

    async def read_all(self) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': 'ReadAll',
            'lock': self._lock
        }})
        ComSrvError.check_raise(result)
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_to_term(self, term: int, timeout: float) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadToTerm': {
                'term': term,
                'timeout_ms': int(timeout * 1e3)
            }},
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_exact(self, count: int, timeout: float) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadExact': {
                'count': count,
                'timeout_ms': int(timeout * 1e3)
            }},
            'lock': self._lock
        }})
        ComSrvError.check_raise(result)
        data = bytes(result['Bytes']['Data'])
        return data

    async def read_upto(self, count: int) -> bytes:
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'ReadUpTo': count},
            'lock': self._lock
        }})
        ComSrvError.check_raise(result)
        data = bytes(result['Bytes']['Data'])
        return data

    async def cobs_write(self, data):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {'CobsWrite': list(data)},
            'lock': self._lock
        }})
        ComSrvError.check_raise(result)

    async def cobs_read(self, timeout):
        result = await get(self._url, {'Bytes': {
            'addr': self._addr,
            'task': {
                'CobsQuery': int(timeout * 1e3),
            },
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)
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
            },
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)
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
            },
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)

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
            },
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)
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
            },
            'lock': self._lock,
        }})
        ComSrvError.check_raise(result)
        return result['Bytes']['String']
