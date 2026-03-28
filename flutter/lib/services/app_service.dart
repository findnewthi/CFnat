import 'package:flutter/material.dart';

abstract class AppService extends ChangeNotifier {
  StatusData? get status;
  ConfigData? get config;
  bool get connected;
  bool get isLoading;
  bool get isRunning;

  Future<void> fetchStatus();
  Future<void> fetchConfig();
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
  });
  Future<bool> stopService();
  Future<List<LogEntry>> fetchLogs();
  Future<bool> clearLogs();
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