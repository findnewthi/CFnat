import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../screens/main_screen.dart' show LayoutConstants;
import '../services/app_service.dart';

class InfoPanel extends StatefulWidget {
  final AppService service;
  final bool forceVertical;
  
  const InfoPanel({super.key, required this.service, this.forceVertical = false});

  @override
  State<InfoPanel> createState() => _InfoPanelState();
}

class _InfoPanelState extends State<InfoPanel> {
  @override
  Widget build(BuildContext context) {
    return Selector<AppService, (StatusData?, bool)>(
      selector: (_, service) => (service.status, service.connected),
      builder: (context, data, child) {
        final (status, connected) = data;
        if (!connected) {
          return _buildDisconnectedState();
        }

        if (status == null) {
          return const Center(child: CircularProgressIndicator());
        }

        if (!status.running) {
          return _buildIdleState();
        }

        return LayoutBuilder(
          builder: (context, constraints) {
            final canFitTwo = constraints.maxWidth >= LayoutConstants.listSideBySideThreshold && !widget.forceVertical;
            final canSplitVertical = constraints.maxHeight >= LayoutConstants.verticalSplitMinHeight;
            
            return Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                children: [
                  _buildHealthCheckBar(status, constraints),
                  const SizedBox(height: 10),
                  Expanded(
                    child: canFitTwo
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
                              const SizedBox(width: 10),
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
                        : canSplitVertical
                            ? Column(
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
                                  const SizedBox(height: 10),
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
                            : ListView(
                                children: [
                                  SizedBox(
                                    height: 360,
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
                                  const SizedBox(height: 10),
                                  SizedBox(
                                    height: 360,
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
              ),
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

  Widget _buildIdleState() {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.play_circle_outline, size: 64, color: Colors.grey[600]),
          const SizedBox(height: 16),
          Text(
            '等待启动',
            style: TextStyle(fontSize: 16, color: Colors.grey[500]),
          ),
          const SizedBox(height: 8),
          Text(
            '点击"启动"来运行',
            style: TextStyle(fontSize: 12, color: Colors.grey[600]),
          ),
        ],
      ),
    );
  }

  Widget _buildHealthCheckBar(StatusData status, BoxConstraints constraints) {
    return const SizedBox.shrink();
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
    
    return Card(
      elevation: 0,
      clipBehavior: Clip.antiAlias,
      child: Column(
        children: [
        Container(
          padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding * 0.7),
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.15),
            border: Border(bottom: BorderSide(color: Colors.grey[800]!)),
          ),
          child: Center(
            child: Text(
              title,
              style: TextStyle(
                fontSize: titleSize,
                fontWeight: FontWeight.bold,
                color: color,
              ),
            ),
          ),
        ),
        Container(
          padding: EdgeInsets.symmetric(horizontal: padding, vertical: padding * 0.5),
          decoration: BoxDecoration(
            color: Colors.grey[900],
            border: Border(bottom: BorderSide(color: Colors.grey[800]!)),
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
                  key: ValueKey('$title-$count-$target-${ips.length}-${stickyIps.length}'),
                  itemCount: ips.length,
                  itemBuilder: (context, index) {
                    return _buildIpRow(ips[index], stickyIps, padding, ipSize);
                  },
                ),
        ),
        ],
      ),
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