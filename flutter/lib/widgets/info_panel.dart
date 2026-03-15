import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/api_service.dart';

class InfoPanel extends StatefulWidget {
  final ApiService api;
  final bool forceVertical;
  
  const InfoPanel({super.key, required this.api, this.forceVertical = false});

  @override
  State<InfoPanel> createState() => _InfoPanelState();
}

class _InfoPanelState extends State<InfoPanel> with SingleTickerProviderStateMixin {
  late AnimationController _progressController;
  double _targetProgress = 1.0;
  double _displayedProgress = 1.0;

  @override
  void initState() {
    super.initState();
    _progressController = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 1),
    )..addListener(() {
        setState(() {
          _displayedProgress = _displayedProgress + 
              (_targetProgress - _displayedProgress) * _progressController.value;
        });
      });
  }

  @override
  void dispose() {
    _progressController.dispose();
    super.dispose();
  }

  void _updateProgress(double newProgress) {
    if ((_targetProgress - newProgress).abs() > 0.01) {
      _displayedProgress = _targetProgress;
      _targetProgress = newProgress;
      _progressController.forward(from: 0);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Consumer<ApiService>(
      builder: (context, api, child) {
        if (!api.connected) {
          return _buildDisconnectedState();
        }

        final status = api.status;
        if (status == null) {
          return const Center(child: CircularProgressIndicator());
        }

        return LayoutBuilder(
          builder: (context, constraints) {
            final isWide = constraints.maxWidth > 600 && !widget.forceVertical;
            
            final progress = status.healthCheckInterval > 0
                ? status.nextHealthCheck / status.healthCheckInterval
                : 0.0;
            _updateProgress(progress.clamp(0.0, 1.0));
            
            return Column(
              children: [
                _buildHealthCheckBar(status, constraints),
                const Divider(height: 1),
                Expanded(
                  child: isWide
                      ? Row(
                          children: [
                            Expanded(
                              child: _buildIpList(
                                '负载均衡',
                                status.primaryIps,
                                status.primaryCount,
                                status.primaryTarget,
                                Colors.green,
                                status.stickyIps,
                                constraints,
                              ),
                            ),
                            const VerticalDivider(width: 1),
                            Expanded(
                              child: _buildIpList(
                                '备选列表',
                                status.backupIps,
                                status.backupCount,
                                status.backupTarget,
                                Colors.blue,
                                status.stickyIps,
                                constraints,
                              ),
                            ),
                          ],
                        )
                      : Column(
                          children: [
                            Expanded(
                              flex: (status.primaryCount + status.primaryTarget).clamp(1, 100),
                              child: _buildIpList(
                                '负载均衡',
                                status.primaryIps,
                                status.primaryCount,
                                status.primaryTarget,
                                Colors.green,
                                status.stickyIps,
                                constraints,
                              ),
                            ),
                            const Divider(height: 1),
                            Expanded(
                              flex: (status.backupCount + status.backupTarget).clamp(1, 100),
                              child: _buildIpList(
                                '备选列表',
                                status.backupIps,
                                status.backupCount,
                                status.backupTarget,
                                Colors.blue,
                                status.stickyIps,
                                constraints,
                              ),
                            ),
                          ],
                        ),
                ),
              ],
            );
          },
        );
      },
    );
  }

  Widget _buildDisconnectedState() {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.cloud_off, size: 64, color: Colors.grey[600]),
          const SizedBox(height: 16),
          Text(
            '后端已断开',
            style: TextStyle(fontSize: 16, color: Colors.grey[500]),
          ),
          const SizedBox(height: 8),
          Text(
            '正在自动重连...',
            style: TextStyle(fontSize: 12, color: Colors.grey[600]),
          ),
        ],
      ),
    );
  }

  Widget _buildHealthCheckBar(StatusData status, BoxConstraints constraints) {
    final padding = constraints.maxWidth > 600 ? 16.0 : 12.0;
    final fontSize = constraints.maxWidth > 400 ? 13.0 : 12.0;
    
    if (!status.running) {
      return Container(
        padding: EdgeInsets.all(padding),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.pause_circle, color: Colors.orange[400], size: fontSize + 7),
            SizedBox(width: padding / 2),
            Text(
              '服务已停止',
              style: TextStyle(color: Colors.orange[400], fontWeight: FontWeight.w500, fontSize: fontSize),
            ),
          ],
        ),
      );
    }

    return Container(
      padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding / 2),
      child: Row(
        children: [
          Icon(Icons.health_and_safety, size: fontSize + 5, color: Colors.lightBlue),
          SizedBox(width: padding / 2),
          Text(
            '健康检查',
            style: TextStyle(fontSize: fontSize, fontWeight: FontWeight.w500),
          ),
          SizedBox(width: padding),
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: LinearProgressIndicator(
                value: _displayedProgress.clamp(0.0, 1.0),
                backgroundColor: Colors.grey[700],
                valueColor: AlwaysStoppedAnimation(
                  _displayedProgress > 0.3 ? Colors.lightBlue : Colors.orange,
                ),
                minHeight: 8,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildIpList(
    String title,
    List<IpInfo> ips,
    int count,
    int target,
    Color color,
    List<String> stickyIps,
    BoxConstraints constraints,
  ) {
    final padding = constraints.maxWidth > 600 ? 12.0 : 8.0;
    final titleSize = constraints.maxWidth > 400 ? 14.0 : 13.0;
    final ipSize = constraints.maxWidth > 400 ? 13.0 : 12.0;
    final headerSize = constraints.maxWidth > 400 ? 11.0 : 10.0;
    
    return Column(
      children: [
        Container(
          padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding * 0.7),
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.15),
            border: Border(bottom: BorderSide(color: Colors.grey[700]!)),
          ),
          child: Row(
            children: [
              Text(
                title,
                style: TextStyle(
                  fontSize: titleSize,
                  fontWeight: FontWeight.bold,
                  color: color,
                ),
              ),
              const Spacer(),
              Container(
                padding: EdgeInsets.symmetric(horizontal: padding * 0.7, vertical: 2),
                decoration: BoxDecoration(
                  color: count >= target ? Colors.green : Colors.orange,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  '$count/$target',
                  style: TextStyle(
                    fontSize: headerSize,
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                  ),
                ),
              ),
            ],
          ),
        ),
        Container(
          padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding * 0.5),
          decoration: BoxDecoration(
            color: Colors.grey[850],
            border: Border(bottom: BorderSide(color: Colors.grey[700]!)),
          ),
          child: Row(
            children: [
              Expanded(
                flex: 3,
                child: Text('IP', style: TextStyle(fontSize: headerSize, color: Colors.grey[400])),
              ),
              Expanded(
                flex: 1,
                child: Text('延迟', style: TextStyle(fontSize: headerSize, color: Colors.grey[400]), textAlign: TextAlign.right),
              ),
              Expanded(
                flex: 1,
                child: Text('丢包', style: TextStyle(fontSize: headerSize, color: Colors.grey[400]), textAlign: TextAlign.right),
              ),
              Expanded(
                flex: 1,
                child: Text('采样', style: TextStyle(fontSize: headerSize, color: Colors.grey[400]), textAlign: TextAlign.right),
              ),
            ],
          ),
        ),
        Expanded(
          child: ips.isEmpty
              ? Center(
                  child: Text('暂无数据', style: TextStyle(color: Colors.grey[500], fontSize: ipSize)),
                )
              : ListView.builder(
                  itemCount: ips.length,
                  itemBuilder: (context, index) {
                    return _buildIpRow(ips[index], stickyIps, padding, ipSize);
                  },
                ),
        ),
      ],
    );
  }

  Widget _buildIpRow(IpInfo ip, List<String> stickyIps, double padding, double fontSize) {
    final delayColor = _getDelayColor(ip.delay);
    final lossColor = _getLossColor(ip.loss);
    final isSticky = stickyIps.contains(ip.ip);

    return Container(
      padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding * 0.7),
      decoration: BoxDecoration(
        color: isSticky ? Colors.purple.withValues(alpha: 0.15) : null,
        border: Border(
          bottom: BorderSide(color: Colors.grey[800]!),
          left: isSticky 
              ? const BorderSide(color: Colors.purple, width: 3)
              : BorderSide.none,
        ),
      ),
      child: Row(
        children: [
          Expanded(
            flex: 3,
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          if (isSticky) ...[
                            Icon(Icons.bolt, size: fontSize, color: Colors.purple[300]),
                            SizedBox(width: padding * 0.3),
                          ],
                          Expanded(
                            child: Text(
                              ip.ip,
                              style: TextStyle(
                                fontSize: fontSize,
                                fontWeight: isSticky ? FontWeight.w600 : FontWeight.w500,
                                color: isSticky ? Colors.purple[300] : null,
                              ),
                            ),
                          ),
                        ],
                      ),
                      if (ip.colo != null && ip.colo!.isNotEmpty)
                        Padding(
                          padding: EdgeInsets.only(left: isSticky ? fontSize + padding * 0.3 : 0),
                          child: Text(
                            ip.colo!,
                            style: TextStyle(fontSize: fontSize - 2, color: Colors.grey[400]),
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            flex: 1,
            child: Text(
              ip.delay > 0 ? '${ip.delay.toStringAsFixed(0)}ms' : '-',
              style: TextStyle(fontSize: fontSize - 1, color: delayColor, fontWeight: FontWeight.w500),
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 1,
            child: Text(
              '${(ip.loss * 100).toStringAsFixed(1)}%',
              style: TextStyle(fontSize: fontSize - 1, color: lossColor, fontWeight: FontWeight.w500),
              textAlign: TextAlign.right,
            ),
          ),
          Expanded(
            flex: 1,
            child: Text(
              '${ip.samples}',
              style: TextStyle(fontSize: fontSize - 1),
              textAlign: TextAlign.right,
            ),
          ),
        ],
      ),
    );
  }

  Color _getDelayColor(double delay) {
    if (delay <= 0) return Colors.grey[500]!;
    if (delay < 100) return Colors.green[400]!;
    if (delay < 300) return Colors.orange[400]!;
    return Colors.red[400]!;
  }

  Color _getLossColor(double loss) {
    if (loss < 0.01) return Colors.green[400]!;
    if (loss < 0.05) return Colors.orange[400]!;
    return Colors.red[400]!;
  }
}
