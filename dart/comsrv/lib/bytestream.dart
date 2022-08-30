import 'dart:typed_data';

import 'package:comsrv/comsrv.dart';
import 'package:wsrpc/wsrpc.dart' show Rpc, JsonObject;

abstract class ByteStreamAddress extends Address {}

class FtdiAddress extends ByteStreamAddress {
  final String port;

  FtdiAddress(this.port);

  @override
  String get enumName => "Ftdi";

  @override
  JsonObject toJson() {
    return {"port": port};
  }
}

class SerialAddress extends ByteStreamAddress {
  final String port;

  SerialAddress(this.port);

  @override
  String get enumName => "Serial";

  @override
  JsonObject toJson() {
    return {"port": port};
  }
}

class TcpAddress extends ByteStreamAddress {
  final String host;
  final int port;

  TcpAddress(this.host, this.port);

  @override
  String get enumName => "Tcp";

  @override
  JsonObject toJson() {
    return {"port": port, "host": host};
  }
}

class SerialPortConfig {
  final String config;
  final int baudrate;

  SerialPortConfig(this.baudrate, this.config);

  JsonObject toJson() {
    return {
      "config": config,
      "baudrate": baudrate,
    };
  }
}

abstract class ByteStreamInstrument extends Instrument {}

class FtdiInstrument extends Instrument {
  final FtdiAddress _address;
  final SerialPortConfig portConfig;

  FtdiInstrument(this._address, this.portConfig);

  @override
  Address get address => _address;

  @override
  JsonObject toJson() {
    return {
      "Ftdi": {
        "address": address.toJson(),
        "port_config": portConfig.toJson(),
      }
    };
  }
}

class SerialInstrument extends Instrument {
  final SerialAddress _address;
  final SerialPortConfig portConfig;

  SerialInstrument(this._address, this.portConfig);

  @override
  Address get address => _address;

  @override
  JsonObject toJson() {
    return {
      "Serial": {
        "address": _address.toJson(),
        "port_config": portConfig.toJson()
      }
    };
  }
}

class TcpInstrument extends Instrument {
  final TcpAddress _address;

  TcpInstrument(this._address);

  @override
  Address get address => _address;

  @override
  JsonObject toJson() {
    return {
      "Tcp": {"address": _address.toJson()}
    };
  }
}

class ByteStreamDevice extends BasePipe {
  ByteStreamInstrument instrument;

  ByteStreamDevice(this.instrument, rpc) : super(rpc);

  @override
  Future<JsonObject> request(JsonObject request) async {
    final result = await super.request({
      "Bytes": {
        "instrument": instrument.toJson(),
        "request": request,
      }
    });
    return result["Bytes"] as JsonObject;
  }

  Future<void> write(Uint8List data) async {
    await request({"Write": data.toList()});
  }

  Future<Uint8List> readAll() async {
    final result = await request({"ReadAll": null});
    return Uint8List.fromList(result["Data"]);
  }

  Future<Uint8List> readToTerm(int term, Duration timeout) async {
    assert(0 <= term && term <= 255);
    final result = await request({
      "ReadToTerm": {
        "term": term,
        "timeout": timeout.toJson(),
      }
    });
    return Uint8List.fromList(result["Data"]);
  }
}
