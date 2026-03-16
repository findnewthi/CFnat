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

class _InfoPanelState extends State<InfoPanel> {
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
            final canSplitVertical = constraints.maxHeight >= 720;
            
            return Padding(
              padding: const EdgeInsets.all(12),
              child: Column(
                children: [
                  _buildHealthCheckBar(status, constraints),
                  const SizedBox(height: 10),
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
                                  const SizedBox(height: 10),
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
    final icon = title == '负载均衡' ? Icons.swap_horiz : Icons.backup;
    
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
          child: Row(
            children: [
              Icon(icon, color: color, size: titleSize),
              const SizedBox(width: 6),
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