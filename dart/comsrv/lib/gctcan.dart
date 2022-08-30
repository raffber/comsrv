import 'dart:typed_data';

const msgTypeSysCtrl = 1;
const msgTypeMonitoringData = 7;
const msgTypeMonitoringRequest = 8;
const msgTypeDdp = 12;
const msgTypeHeartBeat = 14;
const maxDdpDataLenV1 = 61; // 8 message * 8bytes - crc - cmd
const maxDdpDataLenV2 = 8 * 256 - 3; // 256 message * 8bytes - crc - cmd

const broadcastAddress = 0x7F;

abstract class GctMessage {
  int get source;
  int get destination;
  int get messageType;
  int get typeData;
}

enum SysCtrlType { none, value, query }

extension on SysCtrlType {
  int get typeData {
    switch (this) {
      case SysCtrlType.none:
        return 0;
      case SysCtrlType.value:
        return 2;
      case SysCtrlType.query:
        return 1;
    }
  }
}

class SysCtrlMessage extends GctMessage {
  final SysCtrlType sysCtrlType;
  final int _source;
  final int _destination;
  final int command;

  SysCtrlMessage(
      this._source, this._destination, this.command, this.sysCtrlType);

  @override
  int get destination => _destination;

  @override
  int get messageType => msgTypeSysCtrl;

  @override
  int get source => _source;

  @override
  int get typeData => (command << 2) | sysCtrlType.typeData;
}

class MonitoringData extends GctMessage {
  final int _source;
  final int groupIndex;
  final int readingIndex;
  final Uint8List data;

  MonitoringData(this._source, this.groupIndex, this.readingIndex, this.data);

  @override
  int get destination => broadcastAddress;

  @override
  int get messageType => msgTypeMonitoringData;

  @override
  int get source => _source;

  @override
  int get typeData => (groupIndex << 6) | readingIndex;
}
