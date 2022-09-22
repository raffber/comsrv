import 'dart:typed_data';

import 'package:broadcast_wsrpc/lib.dart';
import 'package:comsrv/can.dart';

const msgTypeSysCtrl = 1;
const msgTypeMonitoringData = 7;
const msgTypeMonitoringRequest = 8;
const msgTypeDdp = 12;
const msgTypeHeartBeat = 14;
const maxDdpDataLenV1 = 61; // 8 message * 8bytes - crc - cmd
const maxDdpDataLenV2 = 8 * 256 - 3; // 256 message * 8bytes - crc - cmd

const broadcastAddress = 0x7F;

class MonitorValue {
  final int readingIndex;
  final int groupIndex;
  final Uint8List data;

  MonitorValue(this.readingIndex, this.groupIndex, this.data);
}

class GctCanDevice {
  int controllerId;
  final CanBus canBus;
  Duration timeout = Duration(milliseconds: 100);

  GctCanDevice(this.canBus, this.controllerId);

  Future<List<MonitorValue>> _receiveMonitorRequests(int destinationNodeId,
      int groupIndex, Uint8List readings, Stream<GctMessage> messages) async {
    final receivedReadings = <int, MonitorValue?>{};
    for (final reading in readings) {
      receivedReadings[reading] = null;
    }
    await for (final msg in messages) {
      if (msg is! MonitoringData ||
          msg.groupIndex != groupIndex ||
          msg.source != destinationNodeId ||
          !readings.contains(msg.readingIndex)) {
        continue;
      }
      final value = MonitorValue(msg.readingIndex, msg.groupIndex, msg.data);
      receivedReadings[msg.readingIndex] = value;
      if (!receivedReadings.containsValue(null)) {
        break;
      }
    }
    final ret = <MonitorValue>[];
    for (final reading in readings) {
      ret.add(receivedReadings[reading]!);
    }
    return ret;
  }

  Future<List<MonitorValue>> monitorRequest(
      int destinationNodeId, int groupIndex, Uint8List readings) async {
    final messages = canBus.gctMessages();
    canBus.sendGct(MonitoringRequest(
        source: controllerId,
        destination: destinationNodeId,
        groupIndex: groupIndex,
        readings: readings));
    return await _receiveMonitorRequests(
            destinationNodeId, groupIndex, readings, messages)
        .timeout(timeout);
  }

  Future<void> sysctrlWrite(
      int destinationNodeId, int command, Uint8List data) async {
    final txMsg = SysCtrlMessage(
        controllerId, destinationNodeId, command, SysCtrlType.none, data);
    await canBus.sendGct(txMsg);
  }

  Future<Uint8List> sysctrlRead(int destinationNodeId, int command) async {
    return await sysctrlWriteRead(destinationNodeId, command, Uint8List(0));
  }

  Future<Uint8List> sysctrlWriteRead(
      int destinationNodeId, int command, Uint8List data) async {
    final sysctrlMessages = canBus
        .gctMessages()
        .where((event) => event is SysCtrlMessage)
        .map((event) => event as SysCtrlMessage)
        .where((event) =>
            event.source == destinationNodeId &&
            event.command == command &&
            event.sysCtrlType == SysCtrlType.value);
    final txMsg = SysCtrlMessage(
        controllerId, destinationNodeId, command, SysCtrlType.query, data);
    await canBus.sendGct(txMsg);
    final retMsg = await sysctrlMessages.first.timeout(timeout);
    return retMsg.data;
  }

  Future<Uint8List> ddp(int destinationNodeId, Uint8List data,
      {int version = 2}) async {
    final ddpMessages = canBus
        .gctMessages()
        .where((event) => event is DdpMessage)
        .map((event) => event as DdpMessage)
        .where((event) =>
            event.source == destinationNodeId &&
            event.destination == controllerId &&
            event.version == version);
    await canBus
        .sendGct(DdpMessage(controllerId, destinationNodeId, data, version));
    final retMsg = await ddpMessages.first.timeout(timeout);
    return retMsg.data;
  }
}

abstract class GctMessage {
  int get source;
  int get destination;
  int get messageType;

  JsonObject toJson();

  static GctMessage? fromJson(JsonObject object) {
    if (object.containsKey("SysCtrl")) {
      return SysCtrlMessage.fromJson(object["SysCtrl"]);
    } else if (object.containsKey("MonitoringData")) {
      return MonitoringData.fromJson(object["MonitoringData"]);
    } else if (object.containsKey("MonitoringRequest")) {
      return MonitoringRequest.fromJson(object["MonitoringRequest"]);
    } else if (object.containsKey("Ddp")) {
      return DdpMessage.fromJson(object["Ddp"]);
    } else if (object.containsKey("Heartbeat")) {
      return Heartbeat.fromJson(object["Heartbeat"]);
    }
    return null;
  }
}

enum SysCtrlType { none, value, query }

class SysCtrlMessage extends GctMessage {
  final SysCtrlType sysCtrlType;
  final int _source;
  final int _destination;
  final int command;
  final Uint8List data;

  SysCtrlMessage(this._source, this._destination, this.command,
      this.sysCtrlType, this.data);

  factory SysCtrlMessage.fromJson(JsonObject msg) {
    final source = msg["src"];
    final destination = msg["dst"];
    final command = msg["cmd"];

    final tp = msg["tp"] as String;
    SysCtrlType? sysCtrlType;
    switch (tp) {
      case "None":
        sysCtrlType = SysCtrlType.none;
        break;
      case "Value":
        sysCtrlType = SysCtrlType.value;
        break;
      case "Query":
        sysCtrlType = SysCtrlType.query;
        break;
      default:
        throw ArgumentError("No such SysCtrl type");
    }
    final data = Uint8List.fromList(msg["data"].cast<int>());
    return SysCtrlMessage(source, destination, command, sysCtrlType, data);
  }

  @override
  int get destination => _destination;

  @override
  int get messageType => msgTypeSysCtrl;

  @override
  int get source => _source;

  @override
  JsonObject toJson() {
    String? tp;
    switch (sysCtrlType) {
      case SysCtrlType.none:
        tp = "None";
        break;
      case SysCtrlType.value:
        tp = "Value";
        break;
      case SysCtrlType.query:
        tp = "Query";
        break;
    }
    return {
      "SysCtrl": {
        "src": _source,
        "dst": _destination,
        "cmd": command,
        "tp": tp,
        "data": data.toList(),
      }
    };
  }
}

class MonitoringData extends GctMessage {
  final int _source;
  final int groupIndex;
  final int readingIndex;
  final Uint8List data;

  MonitoringData(this._source, this.groupIndex, this.readingIndex, this.data);

  factory MonitoringData.fromJson(JsonObject object) {
    final source = object["src"];
    final groupIndex = object["group_idx"];
    final readingIndex = object["readings_idx"];
    final data = Uint8List.fromList(object["data"]);
    return MonitoringData(source, groupIndex, readingIndex, data);
  }

  @override
  int get destination => broadcastAddress;

  @override
  int get messageType => msgTypeMonitoringData;

  @override
  int get source => _source;

  @override
  JsonObject toJson() {
    return {
      "MonitoringData": {
        "src": source,
        "group_idx": groupIndex,
        "reading_idx": readingIndex,
        "data": data.toList(),
      }
    };
  }
}

class MonitoringRequest extends GctMessage {
  int groupIndex;

  final int _source;
  final int _destination;
  final Uint8List readings;

  factory MonitoringRequest.fromJson(JsonObject object) {
    final int destination = object["dst"];
    final int source = object["source"];
    final readings = Uint8List.fromList(object["readings"]);
    final groupIndex = object["group_idx"];
    return MonitoringRequest(
      source: source,
      destination: destination,
      groupIndex: groupIndex,
      readings: readings,
    );
  }

  MonitoringRequest({
    required source,
    required destination,
    required this.groupIndex,
    required this.readings,
  })  : _source = source,
        _destination = destination;

  @override
  int get destination => _destination;

  @override
  int get source => _source;

  @override
  int get messageType => msgTypeMonitoringRequest;

  @override
  JsonObject toJson() {
    return {
      "MonitoringRequest": {
        "src": _source,
        "dst": _destination,
        "group_idx": groupIndex,
        "readings": readings,
      }
    };
  }
}

class Heartbeat extends GctMessage {
  final int _source;
  final int productId;

  Heartbeat(this._source, this.productId);

  factory Heartbeat.fromJson(JsonObject object) {
    return Heartbeat(object["src"], object["product_id"]);
  }

  @override
  int get destination => broadcastAddress;

  @override
  int get messageType => msgTypeHeartBeat;

  @override
  int get source => _source;

  @override
  JsonObject toJson() {
    return {
      "Heartbeat": {
        "src": _source,
        "product_id": productId,
      }
    };
  }
}

class DdpMessage extends GctMessage {
  final int _destination;
  final int _source;
  Uint8List data;
  int version;

  DdpMessage(this._source, this._destination, this.data, this.version);

  factory DdpMessage.fromJson(JsonObject object) {
    final int source = object["src"];
    final int destination = object["dst"];
    final int version = object["version"];
    final List<int> data = object["data"];

    return DdpMessage(source, destination, Uint8List.fromList(data), version);
  }

  @override
  int get destination => _destination;

  @override
  int get messageType => msgTypeDdp;

  @override
  int get source => _source;

  @override
  JsonObject toJson() {
    return {
      "Ddp": {
        "version": version,
        "src": _source,
        "dst": _destination,
        "data": data.toList(),
      }
    };
  }
}
