import 'dart:async';
import 'package:flutter/material.dart';
import '../bridge_generated.dart/lib.dart' as rust;
import 'api_service.dart';

class RustService extends ChangeNotifier {
  StatusData? _status;
  ConfigData? _config;
  bool _initialized = false;
  String? _error;
  Timer? _pollTimer;

  StatusData? get status => _status;
  ConfigData? get config => _config;
  bool get initialized => _initialized;
  String? get error => _error;
  bool get connected => _initialized;
  bool get isRunning => _status?.running ?? false;
  bool get isLoading => false;

  Future<void> initialize() async {
    try {
      _initialized = true;
      _startPolling();
      notifyListeners();
    } catch (e) {
      _error = e.toString();
      debugPrint('Rust init error: $_error');
      notifyListeners();
    }
  }

  void _startPolling() {
    _pollTimer?.cancel();
    _pollTimer = Timer.periodic(const Duration(seconds: 1), (_) async {
      if (_initialized) {
        await fetchStatus();
      }
    });
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> fetchStatus() async {
    try {
      final result = await rust.getStatus();
      _status = StatusData(
        running: result.running,
        uptimeSecs: result.uptimeSecs.toInt(),
        nextHealthCheck: result.nextHealthCheck.toInt(),
        healthCheckInterval: result.healthCheckInterval.toInt(),
        primaryCount: result.primaryCount,
        primaryTarget: result.primaryTarget,
        backupCount: result.backupCount,
        backupTarget: result.backupTarget,
        stickyIps: result.stickyIps,
        primaryIps: result.primaryIps.map((e) => IpInfo(
          ip: e.ip,
          colo: e.colo,
          delay: e.delay,
          loss: e.loss,
          samples: e.samples,
        )).toList(),
        backupIps: result.backupIps.map((e) => IpInfo(
          ip: e.ip,
          colo: e.colo,
          delay: e.delay,
          loss: e.loss,
          samples: e.samples,
        )).toList(),
      );
      notifyListeners();
    } catch (e) {
      debugPrint('获取状态失败: $e');
    }
  }

  Future<void> fetchConfig() async {
    try {
      final result = await rust.getConfig();
      _config = ConfigData(
        addr: result.listenAddr,
        delayLimit: result.delayLimit.toInt(),
        tlr: result.tlr,
        ips: result.ips,
        threads: result.threads,
        tlsPort: result.tlsPort,
        httpPort: result.httpPort,
        http: result.http,
        ipFile: result.ipFile,
        maxStickySlots: result.maxStickySlots,
      );
      notifyListeners();
    } catch (e) {
      debugPrint('获取配置失败: $e');
    }
  }

  Future<bool> startService({
    String? ipFile,
    String? http,
    int? delayLimit,
    double? tlr,
    int? ips,
    int? threads,
    int? tlsPort,
    int? httpPort,
    List<String>? colo,
    String? listenAddr,
    int? maxStickySlots,
  }) async {
    try {
      final success = await rust.startService(
        ipFile: ipFile,
        http: http,
        delayLimit: delayLimit != null ? BigInt.from(delayLimit) : null,
        tlr: tlr,
        ips: ips,
        threads: threads,
        tlsPort: tlsPort,
        httpPort: httpPort,
        maxStickySlots: maxStickySlots,
        listenAddr: listenAddr,
      );
      if (success) {
        await fetchConfig();
      }
      return success;
    } catch (e) {
      debugPrint('启动服务失败: $e');
      return false;
    }
  }

  Future<bool> stopService() async {
    try {
      final success = await rust.stopService();
      if (success) {
        _status = StatusData.stopped();
        notifyListeners();
      }
      return success;
    } catch (e) {
      debugPrint('停止服务失败: $e');
      return false;
    }
  }

  Future<List<LogEntry>> fetchLogs() async {
    try {
      final logs = await rust.getLogs();
      return logs.map((e) => LogEntry(
        timestamp: e.timestamp,
        level: e.level,
        message: e.message,
      )).toList();
    } catch (e) {
      debugPrint('获取日志失败: $e');
      return [];
    }
  }

  Future<bool> clearLogs() async {
    try {
      await rust.clearLogs();
      return true;
    } catch (e) {
      debugPrint('清空日志失败: $e');
      return false;
    }
  }
}
