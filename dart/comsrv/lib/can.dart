import 'dart:typed_data';

import 'package:broadcast_wsrpc/lib.dart';
import 'package:comsrv/comsrv.dart';
import 'package:comsrv/gctcan.dart';

abstract class CanAddress extends Address {
  @override
  String get enumName => "Can";
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
}

class LoopBackAddress extends CanAddress {
  @override
  JsonObject toJson() {
    return {"Loopback": null};
  }
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

class CanMessage {}

class DataMessage extends CanMessage {
  final int id;
  final bool extendedId;
  final Uint8List data;

  DataMessage(this.id, this.extendedId, this.data);
}

class RemoteMessage extends CanMessage {
  final bool extendedId;
  final int id;
  final int dlc;

  RemoteMessage(this.id, this.extendedId, this.dlc);
}

class CanBus extends BasePipe {
  CanInstrument instrument;

  CanBus(WsRpc rpc, this.instrument) : super(rpc);

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

  Stream<GctMessage> gctMessages() async* {
    throw UnimplementedError();
  }

  Stream<CanMessage> canMessages() async* {
    throw UnimplementedError();
  }

  Stream<RemoteMessage> remoteMessages() async* {
    throw UnimplementedError();
  }

  Stream<DataMessage> dataMessages() async* {
    throw UnimplementedError();
  }
}
