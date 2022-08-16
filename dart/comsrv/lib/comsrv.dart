library comsrv;

import 'dart:convert';

import 'package:uuid/uuid.dart';
import 'package:wsrpc/wsrpc.dart';

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

class Lock {
  UuidValue id;

  Lock(this.id);

  @override
  String toString() {
    return id.toString();
  }
}

extension on Duration {
  JsonObject toJson() {
    final micros = inMicroseconds;
    final fractionalMicros = micros % 1000000;
    final seconds = (micros / 1000000) as int;
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
    final reply = await rpc.request({"Shutdown": null});
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
}
