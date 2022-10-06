library comsrv;

import 'dart:convert';

import 'package:uuid/uuid.dart';
import 'package:broadcast_wsrpc/lib.dart';
export 'package:broadcast_wsrpc/lib.dart';

class ComSrvError implements Exception {
  String message;
  ComSrvError(this.message);

  static ComSrvError fromJson(JsonObject json) {
    return ComSrvError(jsonEncode(json));
  }

  static void checkAndThrow(JsonObject reply) {
    if (reply.containsKey("Error")) {
      ComSrvError.fromJson(reply["Error"]);
    }
  }
}

abstract class Address {
  JsonObject toJson();

  JsonObject toJsonEnum() {
    return {enumName: toJson()};
  }

  String get enumName;
}

abstract class Instrument {
  JsonObject toJson();

  Address get address;
}

class Lock {
  UuidValue id;

  Lock(this.id);

  @override
  String toString() {
    return id.toString();
  }
}

extension DurationToJson on Duration {
  JsonObject toJson() {
    final micros = inMicroseconds;
    final fractionalMicros = micros % 1000000;
    final seconds = (micros / 1000000).round();
    return {
      "micros": fractionalMicros,
      "seconds": seconds,
    };
  }
}

class Version {
  int major;
  int minor;
  int build;

  Version(this.major, this.minor, this.build);

  static Version fromJson(JsonObject json) {
    return Version(json["major"], json["minor"], json["build"]);
  }
}

class ComSrv {
  Rpc rpc;

  ComSrv(this.rpc);

  Future<JsonObject> request(JsonObject request) async {
    final reply = await rpc.request(request);
    ComSrvError.checkAndThrow(reply);
    return reply;
  }

  Future<void> shutdown() async {
    await request({"Shutdown": null});
  }

  Future<void> dropAll() async {
    await request({"DropAll": null});
  }

  Future<void> drop(Address addr, Lock? lock) async {
    await request({
      "Drop": {"addr": addr.toJsonEnum(), "id": lock?.id}
    });
  }

  Future<Version> version() async {
    final reply = await request({
      "Version": null,
    });
    return Version.fromJson(reply["Version"]);
  }

  Future<Lock> lock(Address addr, Duration timeout) async {
    final response = await request({
      "Lock": {"addr": addr.toJsonEnum(), "timeout": timeout.toJson()}
    });
    final id = response["Locked"]["lock_id"] as String;
    return Lock(UuidValue(id));
  }

  Future<void> unlock(Address addr, Lock lock) async {
    await request({
      "Unlock": {"addr": addr.toJsonEnum(), "id": lock.toString()}
    });
  }

  Future<List<String>> listSerialPorts() async {
    final response = await request({"ListSerialPorts": null});
    final ports = (response["SerialPorts"] as List<dynamic>);
    return ports.cast<String>();
  }

  Future<List<FtdiDeviceInfo>> listFtdiDevices() async {
    final response = await request({"ListFtdiDevices": null});
    final ports = (response["FtdiDevices"] as List<dynamic>).cast<JsonObject>();
    return ports.map(FtdiDeviceInfo.fromJson).toList();
  }

  Future<List<CanDeviceInfo>> listCanDevices() async {
    final response = await request({"ListCanDevices": null});
    final ports = (response["CanDevices"] as List<dynamic>).cast<JsonObject>();
    return ports.map(CanDeviceInfo.fromJson).toList();
  }
}

class FtdiDeviceInfo {
  final bool portOpen;
  final int vendorId;
  final int productId;
  final String serialNumber;
  final String description;

  FtdiDeviceInfo({
    required this.portOpen,
    required this.vendorId,
    required this.productId,
    required this.serialNumber,
    required this.description,
  });

  factory FtdiDeviceInfo.fromJson(JsonObject object) {
    return FtdiDeviceInfo(
      portOpen: object['port_open'] as bool,
      vendorId: object['vendor_id'] as int,
      productId: object['product_id'] as int,
      serialNumber: object['serial_number'] as String,
      description: object['description'] as String,
    );
  }
}

enum CanDriverType { socketcan, pcan, unkown }

class CanDeviceInfo {
  CanDriverType driverType;
  String interfaceName;

  CanDeviceInfo(this.interfaceName, this.driverType);

  factory CanDeviceInfo.fromJson(JsonObject object) {
    final interfaceName = object['interface_name'] as String;
    CanDriverType driverType;
    switch (object["driver_type"]) {
      case "SocketCAN":
        driverType = CanDriverType.socketcan;
        break;
      case "PCAN":
        driverType = CanDriverType.pcan;
        break;
      default:
        driverType = CanDriverType.unkown;
        break;
    }
    return CanDeviceInfo(interfaceName, driverType);
  }
}

class BasePipe {
  Rpc rpc;

  BasePipe(this.rpc);

  Future<JsonObject> request(JsonObject request) async {
    final reply = await rpc.request(request);
    ComSrvError.checkAndThrow(reply);
    return reply;
  }
}
