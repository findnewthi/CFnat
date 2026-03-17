import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:http/http.dart';

class ApiService extends ChangeNotifier {
  StatusData? _status;
  ConfigData? _config;
  bool _connected = false;
  bool _isLoading = false;
  StreamSubscription<String>? _streamSubscription;
  Client? _streamClient;
  int _streamGeneration = 0;
  Timer? _reconnectTimer;

  StatusData? get status => _status;
  ConfigData? get config => _config;
  bool get connected => _connected;
  bool get isLoading => _isLoading;
  bool get isRunning => _status?.running ?? false;

  ApiService() {
    _startStreaming();
  }

  void _handleDisconnect() {
    _connected = false;
    _status = null;
    notifyListeners();
  }

  void _scheduleReconnect() {
    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(const Duration(seconds: 1), _startStreaming);
  }

  void _startStreaming() async {
    final generation = ++_streamGeneration;
    try {
      final client = Client();
      _streamClient = client;
      final request = Request('GET', Uri.parse('/api/stream'));
      
      final response = await client.send(request);
      if (generation != _streamGeneration) {
        client.close();
        return;
      }
      
      _streamSubscription = response.stream
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(
            (line) {
              if (generation != _streamGeneration) {
                return;
              }
              if (line.isNotEmpty && line.startsWith('data: ')) {
                try {
                  final jsonStr = line.substring(6);
                  final data = json.decode(jsonStr);
                  
                  if (data['status'] != null) {
                    _status = StatusData.fromJson(data['status']);
                  }
                  if (data['config'] != null) {
                    _config = ConfigData.fromJson(data['config']);
                  }
                  _connected = true;
                  notifyListeners();
                } catch (e) {
                  debugPrint('解析SSE数据失败: $e');
                }
              }
            },
            onError: (e) {
              if (generation != _streamGeneration) {
                return;
              }
              debugPrint('SSE连接错误: $e');
              _handleDisconnect();
              _scheduleReconnect();
            },
            onDone: () {
              if (generation != _streamGeneration) {
                return;
              }
              debugPrint('SSE连接关闭');
              _handleDisconnect();
              _scheduleReconnect();
            },
          );
    } catch (e) {
      debugPrint('启动SSE失败: $e');
      _handleDisconnect();
      _scheduleReconnect();
    }
  }

  @override
  void dispose() {
    _reconnectTimer?.cancel();
    _streamSubscription?.cancel();
    _streamClient?.close();
    super.dispose();
  }

  Future<void> _restartStreaming() async {
    _streamGeneration++;
    _reconnectTimer?.cancel();
    await _streamSubscription?.cancel();
    _streamClient?.close();
    _streamSubscription = null;
    _streamClient = null;
    _startStreaming();
  }

  Future<void> fetchStatus() async {
    _isLoading = true;
    notifyListeners();

    try {
      final response = await get(Uri.parse('/api/status'));
      if (response.statusCode == 200) {
        _status = StatusData.fromJson(json.decode(response.body));
        _connected = true;
      }
    } catch (e) {
      debugPrint('获取状态失败: $e');
      _handleDisconnect();
    }

    _isLoading = false;
    notifyListeners();
  }

  Future<void> fetchConfig() async {
    try {
      final response = await get(Uri.parse('/api/config'));
      if (response.statusCode == 200) {
        _config = ConfigData.fromJson(json.decode(response.body));
        _connected = true;
      }
    } catch (e) {
      debugPrint('获取配置失败: $e');
    }
    notifyListeners();
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
      final body = <String, dynamic>{
        if (ipFile != null) 'ip_file': ipFile,
        if (http != null) 'http': http,
        if (delayLimit != null) 'delay_limit': delayLimit,
        if (tlr != null) 'tlr': tlr,
        if (ips != null) 'ips': ips,
        if (threads != null) 'threads': threads,
        if (tlsPort != null) 'tls_port': tlsPort,
        if (httpPort != null) 'http_port': httpPort,
        if (colo != null) 'colo': colo,
        if (listenAddr != null) 'listen_addr': listenAddr,
        if (maxStickySlots != null) 'max_sticky_slots': maxStickySlots,
      };

      final response = await post(
        Uri.parse('/api/start'),
        headers: {'Content-Type': 'application/json'},
        body: json.encode(body),
      );

      if (response.statusCode == 200) {
        final result = json.decode(response.body);
        final success = result['success'] == true;
        if (success) {
          await _restartStreaming();
          await fetchConfig();
        }
        return success;
      }
      return false;
    } catch (e) {
      debugPrint('启动服务失败: $e');
      return false;
    }
  }

  Future<bool> stopService() async {
    try {
      final response = await post(Uri.parse('/api/stop'));
      if (response.statusCode == 200) {
        final result = json.decode(response.body);
        final success = result['success'] == true;
        if (success) {
          _status = StatusData.stopped();
          notifyListeners();
          await _restartStreaming();
        }
        return success;
      }
      return false;
    } catch (e) {
      debugPrint('停止服务失败: $e');
      return false;
    }
  }

  Future<List<LogEntry>> fetchLogs() async {
    try {
      final response = await get(Uri.parse('/api/logs'));
      if (response.statusCode == 200) {
        final List<dynamic> data = json.decode(response.body);
        return data.map((e) => LogEntry.fromJson(e)).toList();
      }
      return [];
    } catch (e) {
      debugPrint('获取日志失败: $e');
      return [];
    }
  }

  Future<bool> clearLogs() async {
    try {
      final response = await post(Uri.parse('/api/logs/clear'));
      if (response.statusCode == 200) {
        final result = json.decode(response.body);
        return result['success'] == true;
      }
      return false;
    } catch (e) {
      debugPrint('清空日志失败: $e');
      return false;
    }
  }
}

class StatusData {
  final bool running;
  final int uptimeSecs;
  final int nextHealthCheck;
  final int healthCheckInterval;
  final int primaryCount;
  final int primaryTarget;
  final int backupCount;
  final int backupTarget;
  final List<String> stickyIps;
  final List<IpInfo> primaryIps;
  final List<IpInfo> backupIps;

  StatusData({
    required this.running,
    required this.uptimeSecs,
    required this.nextHealthCheck,
    required this.healthCheckInterval,
    required this.primaryCount,
    required this.primaryTarget,
    required this.backupCount,
    required this.backupTarget,
    required this.stickyIps,
    required this.primaryIps,
    required this.backupIps,
  });

  factory StatusData.stopped() {
    return StatusData(
      running: false,
      uptimeSecs: 0,
      nextHealthCheck: 0,
      healthCheckInterval: 25,
      primaryCount: 0,
      primaryTarget: 0,
      backupCount: 0,
      backupTarget: 0,
      stickyIps: const [],
      primaryIps: const [],
      backupIps: const [],
    );
  }

  factory StatusData.fromJson(Map<String, dynamic> json) {
    return StatusData(
      running: json['running'] ?? false,
      uptimeSecs: json['uptime_secs'] ?? 0,
      nextHealthCheck: json['next_health_check'] ?? 0,
      healthCheckInterval: json['health_check_interval'] ?? 25,
      primaryCount: json['primary_count'] ?? 0,
      primaryTarget: json['primary_target'] ?? 0,
      backupCount: json['backup_count'] ?? 0,
      backupTarget: json['backup_target'] ?? 0,
      stickyIps: (json['sticky_ips'] as List?)
          ?.map((e) => e as String)
          .toList() ?? [],
      primaryIps: (json['primary_ips'] as List?)
          ?.map((e) => IpInfo.fromJson(e))
          .toList() ?? [],
      backupIps: (json['backup_ips'] as List?)
          ?.map((e) => IpInfo.fromJson(e))
          .toList() ?? [],
    );
  }
}

class IpInfo {
  final String ip;
  final String? colo;
  final double delay;
  final double loss;
  final int samples;

  IpInfo({
    required this.ip,
    this.colo,
    required this.delay,
    required this.loss,
    required this.samples,
  });

  factory IpInfo.fromJson(Map<String, dynamic> json) {
    return IpInfo(
      ip: json['ip'] ?? '',
      colo: json['colo'],
      delay: (json['delay'] ?? 0).toDouble(),
      loss: (json['loss'] ?? 0).toDouble(),
      samples: json['samples'] ?? 0,
    );
  }
}

class LogEntry {
  final String timestamp;
  final String level;
  final String message;

  LogEntry({
    required this.timestamp,
    required this.level,
    required this.message,
  });

  factory LogEntry.fromJson(Map<String, dynamic> json) {
    return LogEntry(
      timestamp: json['timestamp'] ?? '',
      level: json['level'] ?? 'INFO',
      message: json['message'] ?? '',
    );
  }
}

class ConfigData {
  final String addr;
  final int delayLimit;
  final double tlr;
  final int ips;
  final int threads;
  final int tlsPort;
  final int httpPort;
  final List<String>? colo;
  final String http;
  final String ipFile;
  final int maxStickySlots;

  ConfigData({
    required this.addr,
    required this.delayLimit,
    required this.tlr,
    required this.ips,
    required this.threads,
    required this.tlsPort,
    required this.httpPort,
    this.colo,
    required this.http,
    required this.ipFile,
    required this.maxStickySlots,
  });

  factory ConfigData.fromJson(Map<String, dynamic> json) {
    return ConfigData(
      addr: json['addr'] ?? '',
      delayLimit: json['delay_limit'] ?? 500,
      tlr: (json['tlr'] ?? 0.1).toDouble(),
      ips: json['ips'] ?? 10,
      threads: json['threads'] ?? 16,
      tlsPort: json['tls_port'] ?? 443,
      httpPort: json['http_port'] ?? 80,
      colo: json['colo'] != null
          ? List<String>.from(json['colo'])
          : null,
      http: json['http'] ?? '',
      ipFile: json['ip_file'] ?? '',
      maxStickySlots: json['max_sticky_slots'] ?? 5,
    );
  }
}