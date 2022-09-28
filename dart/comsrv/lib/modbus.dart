import 'dart:typed_data';

import 'package:broadcast_wsrpc/lib.dart';
import 'package:comsrv/bytestream.dart';
import 'package:comsrv/comsrv.dart';

enum ModBusProtocol { tcp, rtu }

extension on ModBusProtocol {
  String toJson() {
    switch (this) {
      case ModBusProtocol.tcp:
        return "Tcp";
      case ModBusProtocol.rtu:
        return "Rtu";
    }
  }
}

class ModBusDevice {
  ByteStreamPipe pipe;
  int stationAddress;
  ModBusProtocol protocol;
  Duration timeout;

  ModBusDevice(this.pipe, this.stationAddress, this.protocol,
      {this.timeout = const Duration(seconds: 1)});

  Future<JsonObject> request(JsonObject data) async {
    final result = await pipe.request({
      "ModBus": {
        "timeout": timeout.toJson(),
        "station_address": stationAddress,
        "protocol": protocol.toJson(),
        "request": data,
      }
    });
    final ret = result["ModBus"];
    if (ret is String) {
      return {ret: null};
    }
    return ret;
  }

  Future<List<bool>> readCoil(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadCoil": {"addr": address, "cnt": count}
    });
    return result["Bool"] as List<bool>;
  }

  Future<List<bool>> readDiscrete(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadDiscrete": {"addr": address, "cnt": count}
    });
    return result["Bool"] as List<bool>;
  }

  Future<Uint16List> readInput(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadInput": {"addr": address, "cnt": count}
    });
    final resultList = result["Number"] as List<int>;
    return Uint16List.fromList(resultList);
  }

  Future<Uint16List> readHolding(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadHolding": {"addr": address, "cnt": count}
    });
    final resultList = result["Number"].cast<int>();
    return Uint16List.fromList(resultList);
  }

  Future<void> writeRegisters(int address, Uint16List data) async {
    assert(address < (1 << 16));
    await request({
      "WriteRegisters": {"addr": address, "values": data}
    });
  }

  Future<void> writeCoils(int address, Uint16List data) async {
    assert(address < (1 << 16));
    await request({
      "WriteCoils": {"addr": address, "values": data}
    });
  }

  Future<Uint8List> ddp(
      int subCommand, int ddpCommand, bool wantResponse, Uint8List data) async {
    final response = await request({
      "Ddp": {
        "sub_cmd": subCommand,
        "ddp_cmd": ddpCommand,
        "response": wantResponse,
        "data": data.toList()
      }
    });
    return Uint8List.fromList(response["Data"].cast<int>());
  }
}
