import 'comsrv.dart';

class SigrokInstrument {
  String address;
  SigrokInstrument(this.address);

  JsonObject toJson() {
    return {"address": address};
  }
}

class DetectedSigrokDevice {
  SigrokInstrument instrument;
  String description;
  DetectedSigrokDevice(this.instrument, this.description);
}

class SigrokResult {
  Map<String, List<bool>> channels;
  List<double> time;
  SigrokResult(this.time, this.channels);
}

class SigrokDevice {
  SigrokInstrument instrument;
  Rpc rpc;

  SigrokDevice(this.instrument, this.rpc);

  Future<SigrokResult> read({
    List<String>? channels,
    double sampleRate = 48e6,
    int? numSamples,
    double? time,
  }) async {
    if (time != null && numSamples != null) {
      throw AssertionError("Specifiy only one of time or numSamples");
    }
    JsonObject? acquire;
    if (time != null) {
      acquire = {"Time": time};
    } else if (numSamples != null) {
      acquire = {"Samples": numSamples};
    } else {
      throw AssertionError("Neither time nor num_samples is given");
    }
    channels = channels ?? [];

    final request = {
      "Sigrok": {
        "instrument": instrument.toJson(),
        "request": {
          "channels": channels,
          "sample_rate": sampleRate,
          "acquire": acquire,
        },
      }
    };

    final reply = await ComSrv(rpc).request(request);

    final data = reply["Sigrok"]["Data"] as JsonObject;
    final tsample = data["tsample"] as double;
    final length = data["length"] as int;
    final t = List<double>.generate(length, (x) => x * tsample);
    final ret = <String, List<bool>>{};
    for (final kv in (data["channels"] as JsonObject).entries) {
      final base = kv.value as List<int>;
      final bools = <bool>[];
      for (final x in base) {
        for (var kBit = 0; kBit < 8; kBit += 1) {
          final isBitset = x & (1 << kBit) != 0;
          bools.add(isBitset);
        }
      }
      ret[kv.key] = bools;
    }
    return SigrokResult(t, ret);
  }

  static Future<List<DetectedSigrokDevice>> list(Rpc rpc) async {
    final reply = await ComSrv(rpc).request({"ListSigrokDevices": null});
    final devices = reply["Sigrok"]["Devices"];

    final ret = <DetectedSigrokDevice>[];
    for (final dev in devices) {
      ret.add(DetectedSigrokDevice(
          SigrokInstrument(dev["addr"] as String), (dev["desc"] as String)));
    }
    return ret;
  }
}
