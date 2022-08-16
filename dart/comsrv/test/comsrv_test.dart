import 'package:comsrv/comsrv.dart';
import 'package:test/test.dart';
import 'package:wsrpc/wsrpc.dart';

void main() {
  group('Basics', () {
    test('Check Version http', () async {
      final comsrv = ComSrv(HttpRpc("http://127.0.0.1:5903"));
      final version = await comsrv.version();
      assert(version.major == 2);
    });

    test('Check Version websockets', () async {
      final comsrv = ComSrv(WsRpc("ws://127.0.0.1:5902"));
      final version = await comsrv.version();
      assert(version.major == 2);
      await comsrv.rpc.close();
    });
  });
}
