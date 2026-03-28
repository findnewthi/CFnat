import 'dart:async';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/app_service.dart';

class ConfigPanel extends StatefulWidget {
  final AppService service;
  final bool compact;
  
  const ConfigPanel({super.key, required this.service, this.compact = false});

  @override
  State<ConfigPanel> createState() => _ConfigPanelState();
}

class _ConfigPanelState extends State<ConfigPanel> {
  final _speedTestFileController = TextEditingController();
  final _dataCenterController = TextEditingController();
  final _delayLimitController = TextEditingController();
  final _tlrController = TextEditingController();
  final _maxConcurrencyController = TextEditingController();
  final _loadCountController = TextEditingController();
  final _testAddressController = TextEditingController();
  final _localServiceController = TextEditingController();
  final _tlsPortController = TextEditingController();
  final _httpPortController = TextEditingController();
  final _maxStickySlotsController = TextEditingController();
  
  bool _initialized = false;
  bool _isRunning = false;
  bool _connected = false;
  bool _actionInProgress = false;

  @override
  void initState() {
    super.initState();
    _initFromConfig();
  }

  @override
  void dispose() {
    _speedTestFileController.dispose();
    _dataCenterController.dispose();
    _delayLimitController.dispose();
    _tlrController.dispose();
    _maxConcurrencyController.dispose();
    _loadCountController.dispose();
    _testAddressController.dispose();
    _localServiceController.dispose();
    _tlsPortController.dispose();
    _httpPortController.dispose();
    _maxStickySlotsController.dispose();
    super.dispose();
  }

  void _initFromConfig() {
    final config = widget.service.config;
    if (config != null && !_initialized) {
      _speedTestFileController.text = config.ipFile;
      _dataCenterController.text = config.colo?.join(',') ?? '';
      _delayLimitController.text = config.delayLimit.toString();
      _tlrController.text = config.tlr.toString();
      _maxConcurrencyController.text = config.threads.toString();
      _loadCountController.text = config.ips.toString();
      _testAddressController.text = config.http;
      _localServiceController.text = config.addr;
      _tlsPortController.text = config.tlsPort.toString();
      _httpPortController.text = config.httpPort.toString();
      _maxStickySlotsController.text = config.maxStickySlots.toString();
      _initialized = true;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Selector<AppService, (bool, bool, ConfigData?)>(
      selector: (_, service) => (service.isRunning, service.connected, service.config),
      builder: (context, data, child) {
        final (isRunning, connected, config) = data;
        _isRunning = isRunning;
        _connected = connected;
        
        if (config != null && !_initialized) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            _initFromConfig();
          });
        }
        
        if (widget.compact) {
          return LayoutBuilder(
            builder: (context, constraints) => _buildCompactLayout(isRunning, connected, constraints),
          );
        }
        
        return LayoutBuilder(
          builder: (context, constraints) => _buildNormalLayout(isRunning, connected, constraints),
        );
      },
    );
  }

  Widget _buildNormalLayout(bool isRunning, bool connected, BoxConstraints constraints) {
    const itemMinWidth = 160.0;
    final canFitTwo = constraints.maxWidth >= itemMinWidth * 2 + 12;
    
    return CustomScrollView(
      slivers: [
        SliverPadding(
          padding: const EdgeInsets.all(16),
          sliver: SliverList(
            delegate: SliverChildListDelegate([
              _buildTextField(
                controller: _speedTestFileController,
                label: '测速文件',
                enabled: !isRunning,
              ),
              const SizedBox(height: 12),
              _buildTwoColumnRow(
                canFitTwo,
                _buildTextField(
                  controller: _delayLimitController,
                  label: '延迟上限 (ms)',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                _buildTextField(
                  controller: _tlrController,
                  label: '丢包率上限',
                  enabled: !isRunning,
                  isDecimal: true,
                ),
              ),
              const SizedBox(height: 12),
              _buildTwoColumnRow(
                canFitTwo,
                _buildTextField(
                  controller: _maxConcurrencyController,
                  label: '测速并发',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                _buildTextField(
                  controller: _dataCenterController,
                  label: '数据中心',
                  enabled: !isRunning,
                ),
              ),
              const SizedBox(height: 12),
              _buildTwoColumnRow(
                canFitTwo,
                _buildTextField(
                  controller: _loadCountController,
                  label: '负载数量',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                _buildTextField(
                  controller: _maxStickySlotsController,
                  label: '最大负载槽数',
                  enabled: !isRunning,
                  isNumber: true,
                ),
              ),
              const SizedBox(height: 12),
              _buildTwoColumnRow(
                canFitTwo,
                _buildTextField(
                  controller: _tlsPortController,
                  label: 'TLS 端口',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                _buildTextField(
                  controller: _httpPortController,
                  label: 'HTTP 端口',
                  enabled: !isRunning,
                  isNumber: true,
                ),
              ),
              const SizedBox(height: 12),
              _buildTextField(
                controller: _testAddressController,
                label: '测速地址',
                enabled: !isRunning,
              ),
              const SizedBox(height: 12),
              _buildTextField(
                controller: _localServiceController,
                label: '本地服务',
                enabled: !isRunning,
              ),
              const SizedBox(height: 16),
              _buildActionButtons(),
              const SizedBox(height: 16),
              _buildStatusCard(),
            ]),
          ),
        ),
        SliverFillRemaining(
          hasScrollBody: false,
          child: ConstrainedBox(
            constraints: const BoxConstraints(minHeight: 250),
            child: _buildLogPanel(),
          ),
        ),
      ],
    );
  }

  Widget _buildTwoColumnRow(bool canFitTwo, Widget left, Widget right) {
    if (canFitTwo) {
      return Row(
        children: [
          Expanded(child: left),
          const SizedBox(width: 12),
          Expanded(child: right),
        ],
      );
    }
    return Column(
      children: [
        left,
        const SizedBox(height: 12),
        right,
      ],
    );
  }

  Widget _buildCompactLayout(bool isRunning, bool connected, BoxConstraints constraints) {
    final width = constraints.maxWidth;
    final padding = (width * 0.03).clamp(8.0, 12.0);
    final spacing = (width * 0.02).clamp(6.0, 10.0);
    final fontSize = (width * 0.035).clamp(12.0, 14.0);
    
    return CustomScrollView(
      slivers: [
        SliverPadding(
          padding: EdgeInsets.all(padding),
          sliver: SliverList(
            delegate: SliverChildListDelegate([
              _buildTextField(
                controller: _speedTestFileController,
                label: '测速文件',
                enabled: !isRunning,
                fontSize: fontSize,
              ),
              SizedBox(height: spacing),
              Row(
                children: [
                  Expanded(
                    child: _buildTextField(
                      controller: _delayLimitController,
                      label: '延迟上限',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                  SizedBox(width: spacing),
                  Expanded(
                    child: _buildTextField(
                      controller: _tlrController,
                      label: '丢包上限',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isDecimal: true,
                    ),
                  ),
                ],
              ),
              SizedBox(height: spacing),
              Row(
                children: [
                  Expanded(
                    child: _buildTextField(
                      controller: _maxConcurrencyController,
                      label: '测速并发',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                  SizedBox(width: spacing),
                  Expanded(
                    child: _buildTextField(
                      controller: _dataCenterController,
                      label: '数据中心',
                      enabled: !isRunning,
                      fontSize: fontSize,
                    ),
                  ),
                ],
              ),
              SizedBox(height: spacing),
              Row(
                children: [
                  Expanded(
                    child: _buildTextField(
                      controller: _loadCountController,
                      label: '负载数量',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                  SizedBox(width: spacing),
                  Expanded(
                    child: _buildTextField(
                      controller: _maxStickySlotsController,
                      label: '最大负载槽数',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                ],
              ),
              SizedBox(height: spacing),
              Row(
                children: [
                  Expanded(
                    child: _buildTextField(
                      controller: _tlsPortController,
                      label: 'TLS 端口',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                  SizedBox(width: spacing),
                  Expanded(
                    child: _buildTextField(
                      controller: _httpPortController,
                      label: 'HTTP 端口',
                      enabled: !isRunning,
                      fontSize: fontSize,
                      isNumber: true,
                    ),
                  ),
                ],
              ),
              SizedBox(height: spacing),
              _buildTextField(
                controller: _testAddressController,
                label: '测速地址',
                enabled: !isRunning,
                fontSize: fontSize,
              ),
              SizedBox(height: spacing),
              _buildTextField(
                controller: _localServiceController,
                label: '本地服务',
                enabled: !isRunning,
                fontSize: fontSize,
              ),
              SizedBox(height: spacing),
              _buildActionButtons(),
              SizedBox(height: spacing),
              _buildStatusCard(),
            ]),
          ),
        ),
        SliverFillRemaining(
          hasScrollBody: false,
          child: ConstrainedBox(
            constraints: const BoxConstraints(minHeight: 200),
            child: _buildLogPanel(),
          ),
        ),
      ],
    );
  }

  Widget _buildTextField({
    required TextEditingController controller,
    required String label,
    bool enabled = true,
    bool isNumber = false,
    bool isDecimal = false,
    double? fontSize,
  }) {
    final size = fontSize ?? 14.0;
    return TextField(
      controller: controller,
      enabled: enabled,
      keyboardType: isDecimal
          ? const TextInputType.numberWithOptions(decimal: true)
          : isNumber
              ? TextInputType.number
              : TextInputType.text,
      style: fontSize != null ? TextStyle(fontSize: size) : null,
      decoration: InputDecoration(
        labelText: label,
        floatingLabelBehavior: FloatingLabelBehavior.always,
        border: const OutlineInputBorder(),
        contentPadding: fontSize != null 
            ? EdgeInsets.symmetric(horizontal: 10, vertical: size * 0.6)
            : const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        labelStyle: fontSize != null ? TextStyle(fontSize: size - 1) : null,
      ),
    );
  }

  Widget _buildActionButtons() {
    if (!_connected) {
      return Center(
        child: Text(
          '等待连接...',
          style: TextStyle(color: Colors.grey[500], fontSize: 12),
        ),
      );
    }

    if (_isRunning) {
      return SizedBox(
        width: double.infinity,
        child: ElevatedButton.icon(
          onPressed: _actionInProgress
              ? null
              : () async {
                  await _runAction(() => widget.service.stopService(), '停止');
                },
          icon: _actionInProgress
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.stop, size: 18),
          label: Text(_actionInProgress ? '停止中...' : '停止'),
          style: ElevatedButton.styleFrom(
            backgroundColor: Colors.red[700],
            foregroundColor: Colors.white,
            padding: const EdgeInsets.symmetric(vertical: 12),
          ),
        ),
      );
    }

    return SizedBox(
      width: double.infinity,
      child: ElevatedButton.icon(
        onPressed: _actionInProgress
            ? null
            : () async {
                await _runAction(
                  () => widget.service.startService(
                    ipFile: _speedTestFileController.text.isNotEmpty 
                        ? _speedTestFileController.text : null,
                    http: _testAddressController.text.isNotEmpty 
                        ? _testAddressController.text : null,
                    delayLimit: int.tryParse(_delayLimitController.text),
                    tlr: double.tryParse(_tlrController.text),
                    ips: int.tryParse(_loadCountController.text),
                    threads: int.tryParse(_maxConcurrencyController.text),
                    tlsPort: int.tryParse(_tlsPortController.text),
                    httpPort: int.tryParse(_httpPortController.text),
                    colo: _dataCenterController.text.isNotEmpty
                        ? _dataCenterController.text.split(',').map((e) => e.trim()).toList()
                        : null,
                    listenAddr: _localServiceController.text.isNotEmpty 
                        ? _localServiceController.text : null,
                    maxStickySlots: int.tryParse(_maxStickySlotsController.text),
                  ),
                  '启动',
                );
              },
        icon: _actionInProgress
            ? const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : const Icon(Icons.play_arrow, size: 18),
        label: Text(_actionInProgress ? '启动中...' : '启动'),
        style: ElevatedButton.styleFrom(
          backgroundColor: Colors.green[700],
          foregroundColor: Colors.white,
          padding: const EdgeInsets.symmetric(vertical: 12),
        ),
      ),
    );
  }

  Widget _buildStatusCard() {
    return Selector<AppService, (StatusData?, bool)>(
      selector: (_, service) => (service.status, service.connected),
      builder: (context, data, child) {
        final (status, connected) = data;
        final isRunning = status?.running ?? false;
        final uptime = status?.uptimeSecs ?? 0;
        final primaryCount = status?.primaryCount ?? 0;
        final primaryTarget = status?.primaryTarget ?? 0;
        final backupCount = status?.backupCount ?? 0;
        final backupTarget = status?.backupTarget ?? 0;
        final stickyCount = status?.stickyIps.length ?? 0;
        
        return Card(
          elevation: 0,
          margin: EdgeInsets.zero,
          clipBehavior: Clip.antiAlias,
          child: Column(
            children: [
              _buildStatusBar(isRunning, connected, isRunning ? uptime : 0),
              Padding(
                padding: const EdgeInsets.all(12),
                child: Column(
                  children: [
                    _buildQueueRow(Icons.dns, primaryCount, primaryTarget, Colors.green),
                    const SizedBox(height: 8),
                    _buildQueueRow(Icons.backup, backupCount, backupTarget, Colors.blue),
                    const SizedBox(height: 8),
                    _buildQueueRow(Icons.push_pin, stickyCount, primaryTarget, Colors.purple),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildStatusBar(bool isRunning, bool connected, int uptime) {
    Color bgColor;
    Color borderColor;
    Color textColor;
    IconData icon;
    String text;

    if (isRunning) {
      bgColor = Colors.green.withValues(alpha: 0.15);
      borderColor = Colors.green[700]!;
      textColor = Colors.green[400]!;
      icon = Icons.play_circle;
      text = _formatUptime(uptime);
    } else if (connected) {
      bgColor = Colors.blue.withValues(alpha: 0.15);
      borderColor = Colors.blue[700]!;
      textColor = Colors.blue[400]!;
      icon = Icons.pause_circle;
      text = '已连接';
    } else {
      bgColor = Colors.orange.withValues(alpha: 0.15);
      borderColor = Colors.orange[700]!;
      textColor = Colors.orange[400]!;
      icon = Icons.warning;
      text = '未连接';
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: bgColor,
        border: Border(bottom: BorderSide(color: borderColor)),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(icon, size: 16, color: textColor),
          const SizedBox(width: 6),
          Text(
            text,
            style: TextStyle(
              fontSize: 12,
              color: textColor,
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildQueueRow(IconData icon, int count, int target, Color color) {
    final progress = target > 0 ? (count / target).clamp(0.0, 1.0) : 0.0;
    
    return Row(
      children: [
        Icon(icon, size: 18, color: color),
        const SizedBox(width: 8),
        Expanded(
          child: ClipRRect(
            borderRadius: BorderRadius.circular(4),
            child: LinearProgressIndicator(
              value: progress,
              backgroundColor: Colors.grey[800],
              valueColor: AlwaysStoppedAnimation(color),
              minHeight: 6,
            ),
          ),
        ),
        const SizedBox(width: 8),
        SizedBox(
          width: 50,
          child: Text(
            '$count/$target',
            style: TextStyle(
              fontSize: 12,
              color: Colors.grey[300],
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
            textAlign: TextAlign.right,
          ),
        ),
      ],
    );
  }

  Widget _buildLogPanel() {
    return _LogPanel(service: widget.service);
  }

  String _formatUptime(int seconds) {
    final h = seconds ~/ 3600;
    final m = (seconds % 3600) ~/ 60;
    final s = seconds % 60;
    return '${h.toString().padLeft(2, '0')}:${m.toString().padLeft(2, '0')}:${s.toString().padLeft(2, '0')}';
  }

  Future<void> _runAction(Future<bool> Function() action, String label) async {
    if (_actionInProgress || !mounted) {
      return;
    }
    setState(() {
      _actionInProgress = true;
    });
    final success = await action();
    if (!mounted) {
      return;
    }
    setState(() {
      _actionInProgress = false;
    });
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(success ? '$label成功' : '$label失败')),
    );
  }
}

class _LogPanel extends StatefulWidget {
  final AppService service;
  
  const _LogPanel({required this.service});

  @override
  State<_LogPanel> createState() => _LogPanelState();
}

class _LogPanelState extends State<_LogPanel> {
  List<LogEntry> _logs = [];
  bool _loading = false;
  Timer? _refreshTimer;

  @override
  void initState() {
    super.initState();
    _fetchLogs();
    _refreshTimer = Timer.periodic(const Duration(seconds: 2), (_) => _fetchLogs());
  }

  @override
  void dispose() {
    _refreshTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchLogs() async {
    if (_loading) return;
    _loading = true;
    final logs = await widget.service.fetchLogs();
    if (mounted) {
      setState(() {
        _logs = logs;
      });
    }
    _loading = false;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.fromLTRB(16, 0, 16, 16),
      decoration: BoxDecoration(
        color: Colors.grey[900],
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            child: Row(
              children: [
                Icon(Icons.article, size: 14, color: Colors.grey[400]),
                const SizedBox(width: 6),
                Text(
                  '日志',
                  style: TextStyle(fontSize: 12, color: Colors.grey[400]),
                ),
                const Spacer(),
                Text(
                  '${_logs.length}/500',
                  style: TextStyle(fontSize: 11, color: Colors.grey[500]),
                ),
                const SizedBox(width: 8),
                InkWell(
                  onTap: () async {
                    await widget.service.clearLogs();
                    await _fetchLogs();
                  },
                  borderRadius: BorderRadius.circular(4),
                  child: Padding(
                    padding: const EdgeInsets.all(4),
                    child: Icon(Icons.delete_outline, size: 16, color: Colors.grey[500]),
                  ),
                ),
              ],
            ),
          ),
          Divider(height: 1, color: Colors.grey[800]),
          Expanded(
            child: ClipRRect(
              borderRadius: const BorderRadius.only(
                bottomLeft: Radius.circular(8),
                bottomRight: Radius.circular(8),
              ),
              child: _logs.isEmpty
                  ? const Padding(
                      padding: EdgeInsets.all(16),
                      child: Center(
                        child: Text('暂无日志'),
                      ),
                    )
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                      itemCount: _logs.length,
                      itemExtent: 22,
                      itemBuilder: (context, index) {
                        return _buildLogItem(_logs[index]);
                      },
                    ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildLogItem(LogEntry log) {
    Color levelColor;
    switch (log.level) {
      case 'ERROR':
        levelColor = Colors.red[400]!;
        break;
      case 'WARN':
        levelColor = Colors.orange[400]!;
        break;
      default:
        levelColor = Colors.grey[500]!;
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Text(
            log.timestamp,
            style: TextStyle(
              fontSize: 10,
              color: Colors.grey[500],
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
          const SizedBox(width: 6),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 4),
            decoration: BoxDecoration(
              color: levelColor.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(2),
            ),
            child: Text(
              log.level.padRight(5),
              style: TextStyle(fontSize: 9, color: levelColor),
            ),
          ),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              log.message,
              style: const TextStyle(fontSize: 10),
              overflow: TextOverflow.ellipsis,
              maxLines: 1,
            ),
          ),
        ],
      ),
    );
  }
}