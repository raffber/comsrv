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
        """
        Connect to the server managing the CAN bus and start listening for RAW and GCT messages.
        :param url: The URL to the communication server
        """
        if self._client.connected:
            return self
        if url is None:
            url = get_default_ws_url()
        await self._client.connect(url)
        await self.listen_gct()
        await self.listen_raw()
        return self

    async def disconnect(self):
        """ Disconnect from the server """
        await self.client.disconnect()

    async def rpc(self, task, rx_reply=True):
        """
        Perform an RPC to the CAN endpoint.

        :param task: The message to be sent to the CAN endpoint.
        :param rx_reply: True if a reply from the server should be awaited
        :return: The return value of the server or None if `rx_reply=False`
        """
        await self.connect()
        msg = {'Can': {'addr': self._device, 'task': task}}
        if not rx_reply:
            await self._client.send_request(msg)
            return None
        resp = await self._client.request(msg)
        if 'Error' in resp:
            if 'Can' in resp['Error']:
                raise CanException(resp['Error']['Can'])
            else:
                raise ComSrvException(resp['Error'])
        if 'Can' not in resp:
            raise ComSrvException('Unexpected wire format')
        return resp['Can']

    async def send(self, msg, rx_reply=True):
        """
        Send a CAN message. The message must be either of type `poke.can.CanMessage`
        or `poke.can.GctMessage`.
        Awaiting the reply from the server is not necessary and setting `rx_reply=False`
        will disable this.

        :param msg: The message to send to the bus
        :param rx_reply: Wait for an acknowledgment of the server
        """
        if isinstance(msg, CanMessage):
            task = {'TxRaw': msg.to_comsrv()}
        elif isinstance(msg, GctMessage):
            task = {'TxGct': msg.to_comsrv()}
        else:
            raise CanException('Invalid message type.')
        await self.rpc(task, rx_reply=rx_reply)

    async def listen_raw(self):
        """ Start listening to raw CAN messages on the bus """
        await self.rpc({'ListenRaw': True})

    async def listen_gct(self):
        """ Start listening to GCT CAN messages on the bus """
        await self.rpc({'ListenGct': True})

    def raw(self):
        """ Return a listener for raw CAN messages """
        return self._client.listen(raw_filter)

    def gct(self):
        """ Return a listener for GCT CAN messages """
        return self._client.listen(gct_filter)

    @property
    def client(self):
        """ Return the underlying RPC client """
        return self._client
