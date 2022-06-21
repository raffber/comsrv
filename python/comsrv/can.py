from typing import Optional

from . import get_default_ws_url, ComSrvError
from pywsrpc.client import Client

from enum import Enum


class CanError(Exception):
    pass


class CanMessage(object):
    def to_comsrv(self):
        raise NotImplementedError

    @classmethod
    def from_comsrv(cls, msg):
        if 'Data' in msg:
            return DataMessage.from_comsrv(msg['Data'])
        if 'Remote' in msg:
            return RemoteMessage.from_comsrv(msg['Remote'])
        raise ValueError('Invalid json object')


class DataMessage(CanMessage):
    """
    A single data message on the CAN bus.

    Fields are the following:
        data: bytes()
        canid: int
        extid: bool
    """

    def __init__(self):
        super().__init__()
        self.data = bytes()
        self.canid = 0
        self.extid = False

    def clone(self):
        ret = DataMessage()
        ret.data = bytes(self.data)
        ret.canid = self.canid
        ret.extid = self.extid
        return ret

    def to_comsrv(self):
        return {'Data': {
            'id': self.canid,
            'ext_id': self.extid,
            'data': list(self.data)
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.canid = msg['id']
        ret.extid = msg['ext_id']
        ret.data = msg['data']
        return ret

    def __repr__(self):
        return '<comsrv.can.DataMessage canid={0:x} ext_id={1} data={2}>'.format(self.canid, self.extid, self.data)


class RemoteMessage(CanMessage):
    """
    A remote frame on the can bus

    Fields are the following:
        dlc: int
        canid: int
        extid: bool
    """

    def __init__(self):
        super().__init__()
        self.dlc = 0
        self.canid = 0
        self.time = 0
        self.extid = False

    def clone(self):
        ret = DataMessage()
        ret.dlc = int(self.dlc)
        ret.canid = self.canid
        ret.time = self.time
        ret.extid = self.extid
        return ret

    def to_comsrv(self):
        return {'Remote': {
            'id': self.canid,
            'ext_id': self.extid,
            'dlc': self.dlc
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.canid = msg['id']
        ret.extid = msg['ext_id']
        ret.dlc = msg['dlc']
        return ret


class GctMessage(object):
    """
    A GCT message. This message may consist of several raw CAN messages on the bus.
    """

    def __init__(self):
        super().__init__()
        self.src = 0

    def to_comsrv(self):
        """
        Encode the message to the JSON RPC format of the comsrv.
        """
        raise NotImplementedError

    @classmethod
    def from_comsrv(cls, msg):
        """
        Decode the message from the comsrv RPC format
        :param msg: The JSON encoded message
        """
        if 'SysCtrl' in msg:
            return SysCtrlMessage.from_comsrv(msg['SysCtrl'])
        if 'MonitoringData' in msg:
            return MonitoringData.from_comsrv(msg['MonitoringData'])
        if 'MonitoringRequest' in msg:
            return MonitoringRequest.from_comsrv(msg['MonitoringRequest'])
        if 'Ddp' in msg:
            return DdpMessage.from_comsrv(msg['Ddp'])
        if 'Heartbeat' in msg:
            return Heartbeat.from_comsrv(msg['Heartbeat'])
        raise ValueError('Invalid json object')


class SysCtrlType(Enum):
    EMPTY = 'None'
    VALUE = 'Value'
    QUERY = 'Query'


class SysCtrlMessage(GctMessage):
    def __init__(self):
        super().__init__()
        self.dst = 0
        self.cmd = 0
        self.tp = SysCtrlType.EMPTY
        self.data = b''

    def to_comsrv(self):
        return {'SysCtrl': {
            'src': self.src,
            'dst': self.dst,
            'cmd': self.cmd,
            'tp': self.tp.value,
            'data': list(self.data),
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.dst = msg['dst']
        ret.src = msg['src']
        ret.cmd = msg['cmd']
        tp = msg['tp']
        if tp == 'None':
            ret.tp = SysCtrlType.EMPTY
        elif tp == 'Value':
            ret.tp = SysCtrlType.VALUE
        elif tp == 'Query':
            ret.tp = SysCtrlType.QUERY
        else:
            raise ValueError('No such SysCtrl type')
        ret.data = bytes(msg['data'])
        return ret


class MonitoringData(GctMessage):
    def __init__(self):
        super().__init__()
        self.group_idx = 0
        self.reading_idx = 0
        self.data = b''

    def to_comsrv(self):
        return {'MonitoringData': {
            'src': self.src,
            'group_idx': self.group_idx,
            'reading_idx': self.reading_idx,
            'data': list(self.data),
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.src = msg['src']
        ret.group_idx = msg['group_idx']
        ret.reading_idx = msg['reading_idx']
        ret.data = bytes(msg['data'])
        return ret

    def __repr__(self):
        return '<comsrv.can.MonitoringData src={:x} group={} readings={} data={}>'.format(self.src, self.group_idx,
                                                                                          self.reading_idx, self.data)


class MonitoringRequest(GctMessage):
    def __init__(self):
        super().__init__()
        self.dst = 0
        self.group_idx = 0
        self.readings = 0

    def to_comsrv(self):
        return {'MonitoringRequest': {
            'src': self.src,
            'dst': self.dst,
            'group_idx': self.group_idx,
            'readings': self.readings,
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.src = msg['src']
        ret.dst = msg['dst']
        ret.group_idx = msg['group_idx']
        ret.readings = msg['readings']
        return ret

    def __repr__(self):
        return '<comsrv.can.MonitoringData dst={:x} group={} readings={}>'.format(self.dst, self.group_idx,
                                                                                  self.readings)


class DdpMessage(GctMessage):
    def __init__(self, version=1):
        super().__init__()
        self.dst = 0
        self.data = []
        self.version = version

    def to_comsrv(self):
        return {'Ddp': {
            'version': self.version,
            'src': self.src,
            'dst': self.dst,
            'data': list(self.data),
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.src = msg['src']
        ret.dst = msg['dst']
        ret.data = bytes(msg['data'])
        ret.version = msg.get('version', 1)
        return ret

    def __repr__(self):
        return '<comsrv.can.DdpMessage v={} src={:x} dst={:x} data={}>'.format(self.version, self.src, self.dst,
                                                                               self.data)


class Heartbeat(GctMessage):
    def __init__(self):
        super().__init__()
        self.product_id = 0

    def to_comsrv(self):
        return {'Heartbeat': {
            'src': self.src,
            'product_id': self.product_id,
        }}

    @classmethod
    def from_comsrv(cls, msg):
        ret = cls()
        ret.src = msg['src']
        ret.product_id = msg['product_id']
        return ret


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

    async def enable_loopback(self, loopback=True):
        await self.rpc({'EnableLoopback': loopback})

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
                raise CanError(resp['Error']['Can'])
            else:
                raise ComSrvError(resp['Error'])
        if 'Can' not in resp:
            raise ComSrvError('Unexpected wire format')
        return resp['Can']

    async def send(self, msg, rx_reply=True):
        """
        Send a CAN message. The message must be either of type `comsrv.can.CanMessage`
        or `comsrv.can.GctMessage`.
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
            raise CanError('Invalid message type.')
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
