import 'dart:typed_data';

import 'package:comsrv/comsrv.dart';
import 'package:broadcast_wsrpc/lib.dart' show JsonObject;

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

class FtdiInstrument extends ByteStreamInstrument {
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

class SerialInstrument extends ByteStreamInstrument {
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

class TcpInstrument extends ByteStreamInstrument {
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

class ByteStreamPipe extends BasePipe {
  ByteStreamInstrument instrument;

  ByteStreamPipe(this.instrument, rpc) : super(rpc);

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

  Future<Uint8List> readExact(int count, Duration timeout) async {
    final result = await request({
      "ReadExact": {"count": count, "timeout": timeout.toJson()},
    });
    return Uint8List.fromList(result["Data"]);
  }

  Future<void> cobsWrite(Uint8List data) async {
    await request({
      "CobsWrite": data.toList(),
    });
  }

  Future<Uint8List> cobsRead(Duration timeout) async {
    final result = await request({
      "CobsRead": timeout.toJson(),
    });
    return Uint8List.fromList(result["Data"]);
  }

  Future<Uint8List> cobsQuery(Uint8List writeData, Duration timeout) async {
    final result = await request({
      "CobsQuery": {
        "data": writeData.toList(),
        "timeout": timeout.toJson(),
      }
    });
    return Uint8List.fromList(result["Data"]);
  }

  Future<void> writeLine(String line, {String term = "\n"}) async {
    await request({
      "WriteLine": {"line": line, "term": term}
    });
  }

  Future<String> readLine(Duration timeout, {String term = "\n"}) async {
    final result = await request({
      "ReadLine": {"term": term, "timeout": timeout.toJson()}
    });
    return result["String"] as String;
  }

  Future<String> queryLIne(String line, Duration timeout,
      {String term = "\n"}) async {
    final result = await request({
      "QueryLine": {
        "line": line,
        "term": term,
        "timeout": timeout.toJson(),
      }
    });
    return result["String"] as String;
  }

  Future<void> connect() async {
    await request({'Connect': null});
  }

  Future<void> disconnect() async {
    await request({'Disconnect': null});
  }
}
