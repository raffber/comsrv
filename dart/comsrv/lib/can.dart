import 'dart:typed_data';
import 'package:quiver/core.dart';

import 'package:comsrv/comsrv.dart';
import 'package:comsrv/gctcan.dart';

abstract class CanAddress extends Address {
  @override
  String get enumName => "Can";

  CanAddress();

  factory CanAddress.fromJson(JsonObject obj) {
    if (obj.containsKey("PCan")) {
      final addr = obj["PCan"];
      return PCanAddress(addr["address"]);
    } else if (obj.containsKey("SocketCan")) {
      final addr = obj["SocketCan"];
      return SocketCanAddress(addr["interface"]);
    } else if (obj.containsKey("UsrCanet")) {
      final addr = obj["UsrCanet"];
      return UsrCanetAddress(addr["host"], addr["port"]);
    } else if (obj.containsKey("Loopback")) {
      return LoopBackAddress();
    }
    throw ArgumentError("Invalid JSON object.");
  }
}

class PCanAddress extends CanAddress {
  final String address;

  PCanAddress(this.address);

  @override
  JsonObject toJson() {
    return {
      "PCan": {"address": address}
    };
  }

  @override
  bool operator ==(Object other) {
    if (other is! PCanAddress) {
      return false;
    }
    return other.address == address;
  }

  @override
  int get hashCode => address.hashCode;
}

class SocketCanAddress extends CanAddress {
  final String interface;

  SocketCanAddress(this.interface);

  @override
  JsonObject toJson() {
    return {
      "SocketCan": {"interface": interface}
    };
  }

  @override
  bool operator ==(Object other) {
    if (other is! SocketCanAddress) {
      return false;
    }
    return other.interface == interface;
  }

  @override
  int get hashCode => interface.hashCode;
}

class UsrCanetAddress extends CanAddress {
  final String host;
  final int port;

  UsrCanetAddress(this.host, this.port) {
    assert(port < (1 << 16));
  }

  @override
  JsonObject toJson() {
    return {
      "UsrCanet": {"host": host, "port": port}
    };
  }

  @override
  bool operator ==(Object other) {
    if (other is! UsrCanetAddress) {
      return false;
    }
    return (other.host == host) && (other.port == port);
  }

  @override
  int get hashCode => hash2(host, port);
}

class LoopBackAddress extends CanAddress {
  @override
  JsonObject toJson() {
    return {"Loopback": null};
  }

  static final LoopBackAddress _singleton = LoopBackAddress._internal();

  factory LoopBackAddress() {
    return _singleton;
  }

  LoopBackAddress._internal();
}

class CanInstrument extends Instrument {
  final CanAddress _address;
  final int bitrate;

  CanInstrument(this._address, {this.bitrate = 0});

  @override
  Address get address => _address;

  @override
  JsonObject toJson() {
    if (_address is PCanAddress) {
      return {
        "PCan": {
          "address": (_address as PCanAddress).address,
          "bitrate": bitrate
        }
      };
    }
    return _address.toJson();
  }
}

abstract class CanMessage {
  CanMessage();

  factory CanMessage.fromJson(JsonObject object) {
    if (object.containsKey("Data")) {
      return DataMessage.fromJson(object["Data"]);
    } else if (object.containsKey("Remote")) {
      return RemoteMessage.fromJson(object["Remote"]);
    }
    throw ArgumentError("Cannot deserialize CanMessage: Invalid format.");
  }

  JsonObject toJson();
}

class DataMessage extends CanMessage {
  final int id;
  final bool extendedId;
  final Uint8List data;

  DataMessage(this.id, this.extendedId, this.data);

  factory DataMessage.fromJson(JsonObject object) {
    final id = object["id"];
    final extendedId = object["ext_id"] as bool;
    final data = Uint8List.fromList(object["data"].cast<int>() as List<int>);
    return DataMessage(id, extendedId, data);
  }

  @override
  JsonObject toJson() {
    return {"id": id, "ext_id": extendedId, "data": data.toList()};
  }
}

class RemoteMessage extends CanMessage {
  final bool extendedId;
  final int id;
  final int dlc;

  RemoteMessage(this.id, this.extendedId, this.dlc);

  factory RemoteMessage.fromJson(JsonObject object) {
    final id = object["id"];
    final extendedId = object["ext_id"] as bool;
    final dlc = object["dlc"] as int;
    return RemoteMessage(id, extendedId, dlc);
  }

  @override
  JsonObject toJson() {
    return {"id": id, "ext_id": extendedId, "dlc": dlc};
  }
}

class CanBus extends BasePipe {
  CanInstrument instrument;
  final WsRpc wsRpc;
  Duration sendTimeout = Duration(milliseconds: 100);

  CanBus(WsRpc rpc, this.instrument)
      : wsRpc = rpc,
        super(rpc);

  @override
  Future<JsonObject> request(JsonObject request) async {
    final msg = {
      "Can": {"instrument": instrument.toJson(), "request": request}
    };
    final reply = await super.request(msg);
    return reply;
  }

  Future<void> connect() async {
    await request({"ListenGct": true});
    await request({"ListenRaw": true});
  }

  Future<void> sendGct(GctMessage msg) async {
    await request({"TxGct": msg.toJson()});
  }

  Future<void> sendRaw(CanMessage msg) async {
    await request({"TxRaw": msg.toJson()});
  }

  Stream<JsonObject> _filterMessages(
      {Future<void> Function()? runBefore}) async* {
    final client = await wsRpc.connect();
    await for (final msg in client.messages(runBefore: runBefore)) {
      if (!msg.containsKey("Notify")) {
        continue;
      }
      final notification = msg["Notify"] as JsonObject;
      if (!notification.containsKey("Can")) {
        continue;
      }
      final canMsg = notification["Can"] as JsonObject;
      final source = canMsg["source"] as JsonObject;
      final sourceAddress = CanAddress.fromJson(source);
      if (sourceAddress != instrument.address) {
        continue;
      }
      final response = canMsg["response"];
      if (response is! JsonObject) {
        continue;
      }
      if (response.containsKey("Gct") || response.containsKey("Raw")) {
        yield response;
      }
    }
  }

  Stream<GctMessage> gctMessages({Future<void> Function()? runBefore}) async* {
    await for (final msg in _filterMessages(runBefore: runBefore)) {
      if (msg.containsKey("Gct")) {
        final gctmsg = GctMessage.fromJson(msg["Gct"]);
        if (gctmsg != null) {
          yield gctmsg;
        }
      }
    }
  }

  Stream<CanMessage> canMessages({Future<void> Function()? runBefore}) async* {
    await for (final msg in _filterMessages(runBefore: runBefore)) {
      if (msg.containsKey("Raw")) {
        yield CanMessage.fromJson(msg["Raw"]);
      }
    }
  }

  Stream<RemoteMessage> remoteMessages(
      {Future<void> Function()? runBefore}) async* {
    await for (final msg in canMessages(runBefore: runBefore)) {
      if (msg is RemoteMessage) {
        yield msg;
      }
    }
  }

  Stream<DataMessage> dataMessages(
      {Future<void> Function()? runBefore}) async* {
    await for (final msg in canMessages(runBefore: runBefore)) {
      if (msg is DataMessage) {
        yield msg;
      }
    }
  }
}
