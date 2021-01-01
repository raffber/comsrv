from typing import Optional

from poke.can import CanException, CanMessage, GctMessage
from poke.comsrv import get_default_ws_url, ComSrvException
from pywsrpc.client import Client


def gct_filter(msg):
    if 'Notify' not in msg:
        return None
    if 'Can' not in msg['Notify']:
        return None
    can = msg['Notify']['Can']
    if 'Gct' in can:
        msg = can['Gct']
        return GctMessage.from_comsrv(msg)
    return None


def raw_filter(msg):
    if 'Notify' not in msg:
        return None
    if 'Can' not in msg['Notify']:
        return None
    can = msg['Notify']['Can']
    if 'Raw' in can:
        msg = can['Raw']
        return CanMessage.from_comsrv(msg)
    return None


class CanBus(object):
    def __init__(self, device: str, client: Optional[Client] = None):
        if client is None:
            client = Client()
        self._client = client
        self._device = device

    async def connect(self, url=None):
        if not self._client.connected:
            return self
        if url is None:
            url = get_default_ws_url()
        await self._client.connect(url)
        return self

    async def rpc(self, task):
        await self.connect()
        resp = await self._client.query({'Can': {'addr': self._device, 'task': task}})
        if 'Error' in resp:
            if 'Can' in resp['Error']:
                raise CanException(resp['Error']['Can'])
            else:
                raise ComSrvException(resp['Error'])
        if 'Can' not in resp:
            raise ComSrvException('Unexpected wire format')
        return resp['Can']

    async def send(self, msg):
        if isinstance(msg, CanMessage):
            task = {'TxRaw': msg.to_comsrv()}
        elif isinstance(msg, GctMessage):
            task = {'TxGct': msg.to_comsrv()}
        else:
            raise CanException('Invalid message type.')
        await self.rpc(task)

    async def listen_raw(self):
        await self.rpc({'ListenRaw': True})

    async def listen_gct(self):
        await self.rpc({'ListenGct': True})

    async def raw(self):
        await self.rpc({'ListenRaw': True})
        return self._client.listen(raw_filter)

    async def gct(self):
        await self.rpc({'ListenGct': True})
        return self._client.listen(gct_filter)

    @property
    def client(self):
        return self._client
