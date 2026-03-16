import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/api_service.dart';

class ConfigPanel extends StatefulWidget {
  final ApiService api;
  final bool compact;
  
  const ConfigPanel({super.key, required this.api, this.compact = false});

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
    super.dispose();
  }

  void _initFromConfig() {
    final config = widget.api.config;
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
      _initialized = true;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Selector<ApiService, (bool, bool, ConfigData?)>(
      selector: (_, api) => (api.isRunning, api.connected, api.config),
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
        
        return _buildNormalLayout(isRunning, connected);
      },
    );
  }

  Widget _buildNormalLayout(bool isRunning, bool connected) {
    return Container(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                '参数设置',
                style: TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.bold,
                ),
              ),
              _buildStatusBadge(isRunning, connected),
            ],
          ),
          const SizedBox(height: 16),
          Expanded(
            child: ListView(
              children: [
                _buildTextField(
                  controller: _speedTestFileController,
                  label: '测速文件',
                  enabled: !isRunning,
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _dataCenterController,
                  label: '数据中心',
                  enabled: !isRunning,
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _delayLimitController,
                  label: '延迟上限 (ms)',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _tlrController,
                  label: '丢包率上限',
                  enabled: !isRunning,
                  isDecimal: true,
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _maxConcurrencyController,
                  label: '最大并发',
                  enabled: !isRunning,
                  isNumber: true,
                ),
                const SizedBox(height: 12),
                _buildTextField(
                  controller: _loadCountController,
                  label: '负载数量',
                  enabled: !isRunning,
                  isNumber: true,
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
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(
                      child: _buildTextField(
                        controller: _tlsPortController,
                        label: 'TLS端口',
                        enabled: !isRunning,
                        isNumber: true,
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: _buildTextField(
                        controller: _httpPortController,
                        label: 'HTTP端口',
                        enabled: !isRunning,
                        isNumber: true,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 24),
                _buildActionButtons(),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildCompactLayout(bool isRunning, bool connected, BoxConstraints constraints) {
    final width = constraints.maxWidth;
    final padding = (width * 0.03).clamp(8.0, 12.0);
    final spacing = (width * 0.02).clamp(6.0, 10.0);
    final fontSize = (width * 0.035).clamp(12.0, 14.0);
    
    return SingleChildScrollView(
      padding: EdgeInsets.all(padding),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                '参数设置',
                style: TextStyle(
                  fontSize: fontSize + 2,
                  fontWeight: FontWeight.bold,
                ),
              ),
              _buildStatusBadge(isRunning, connected),
            ],
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
                  label: '最大并发',
                  enabled: !isRunning,
                  fontSize: fontSize,
                  isNumber: true,
                ),
              ),
              SizedBox(width: spacing),
              Expanded(
                child: _buildTextField(
                  controller: _loadCountController,
                  label: '负载数量',
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
                  label: 'TLS端口',
                  enabled: !isRunning,
                  fontSize: fontSize,
                  isNumber: true,
                ),
              ),
              SizedBox(width: spacing),
              Expanded(
                child: _buildTextField(
                  controller: _httpPortController,
                  label: 'HTTP端口',
                  enabled: !isRunning,
                  fontSize: fontSize,
                  isNumber: true,
                ),
              ),
            ],
          ),
          SizedBox(height: spacing),
          _buildTextField(
            controller: _speedTestFileController,
            label: '测速文件',
            enabled: !isRunning,
            fontSize: fontSize,
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
            controller: _dataCenterController,
            label: '数据中心',
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
        ],
      ),
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

  Widget _buildStatusBadge(bool isRunning, bool connected) {
    if (isRunning) {
      return _buildBadge(Icons.play_circle, '运行中', Colors.green);
    } else if (connected) {
      return _buildBadge(Icons.pause_circle, '已连接', Colors.blue);
    } else {
      return _buildBadge(Icons.warning, '未连接', Colors.orange);
    }
  }

  Widget _buildBadge(IconData icon, String text, MaterialColor color) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(4),
        border: Border.all(color: color[400]!),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 16, color: color[400]),
          const SizedBox(width: 4),
          Text(
            text,
            style: TextStyle(fontSize: 12, color: color[400]),
          ),
        ],
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
                  await _runAction(() => widget.api.stopService(), '停止');
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
                  () => widget.api.startService(
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