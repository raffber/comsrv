import 'dart:typed_data';

import 'package:broadcast_wsrpc/lib.dart';
import 'package:comsrv/bytestream.dart';
import 'package:comsrv/comsrv.dart';

enum ModBusProtocol { Tcp, Rtu }

extension on ModBusProtocol {
  String toJson() {
    switch (this) {
      case ModBusProtocol.Tcp:
        return "Tcp";
      case ModBusProtocol.Rtu:
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
    return result["ModBus"];
  }

  Future<List<bool>> readCoil(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadCoil": {"addr": address, "count": count}
    });
    return result["Bool"] as List<bool>;
  }

  Future<List<bool>> readDiscrete(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadDiscrete": {"addr": address, "count": count}
    });
    return result["Bool"] as List<bool>;
  }

  Future<Uint16List> readInput(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadInput": {"addr": address, "count": count}
    });
    final resultList = result["Number"] as List<int>;
    return Uint16List.fromList(resultList);
  }

  Future<Uint16List> readHolding(int address, int count) async {
    assert(address < (1 << 16));
    assert(count < (1 << 16));
    final result = await request({
      "ReadHolding": {"addr": address, "count": count}
    });
    final resultList = result["Number"] as List<int>;
    return Uint16List.fromList(resultList);
  }

  Future<void> writeRegisters(int address, Uint16List data) async {
    assert(address < (1 << 16));
    await request({
      "WriteRegisters": {"addr": address, "valus": data}
    });
  }

  Future<void> writeCoils(int address, Uint16List data) async {
    assert(address < (1 << 16));
    await request({
      "WriteCoils": {"addr": address, "valus": data}
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
    return Uint8List.fromList(response["Data"] as List<int>);
  }
}
