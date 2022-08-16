library comsrv;

import 'package:uuid/uuid.dart';
import 'package:wsrpc/wsrpc.dart';

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

  Future<void> shutdown() async {
    await rpc.request({"Shutdown": null});
  }

  Future<void> dropAll() async {
    await rpc.request({"DropAll": null});
  }

  Future<void> drop(Address addr, Lock? lock) async {
    await rpc.request({
      "Drop": {"addr": addr.toJsonEnum(), "id": lock?.id}
    });
  }

  Future<Version> version() async {
    final reply = await rpc.request({
      "Version": null,
    });
    return Version.fromJson(reply["Version"]);
  }
}
