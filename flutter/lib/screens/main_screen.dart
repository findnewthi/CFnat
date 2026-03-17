import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/api_service.dart';
import '../widgets/config_panel.dart';
import '../widgets/info_panel.dart';

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('CFnat Manager'),
        centerTitle: true,
      ),
      body: Consumer<ApiService>(
        builder: (context, api, child) {
          return LayoutBuilder(
            builder: (context, constraints) {
              final width = constraints.maxWidth;
              final height = constraints.maxHeight;
              final isNarrow = width < 680 || (width < 900 && height < 540);

              if (isNarrow) {
                return _buildNarrowLayout(api);
              }

              return _buildFlexibleLayout(api);
            },
          );
        },
      ),
    );
  }

  Widget _buildNarrowLayout(ApiService api) {
    final colorScheme = Theme.of(context).colorScheme;
    final dividerColor = Theme.of(context).dividerColor;

    return DefaultTabController(
      length: 2,
      child: Column(
        children: [
          Container(
            decoration: BoxDecoration(
              color: colorScheme.surface.withValues(alpha: 0.35),
              border: Border(bottom: BorderSide(color: dividerColor)),
            ),
            child: const TabBar(
              tabs: [
                Tab(text: '配置'),
                Tab(text: '列表'),
              ],
            ),
          ),
          Expanded(
            child: TabBarView(
              children: [
                ConfigPanel(api: api, compact: true),
                InfoPanel(api: api, forceVertical: true),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildFlexibleLayout(ApiService api) {
    return Row(
      children: [
        Flexible(
          flex: 3,
          fit: FlexFit.loose,
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              minWidth: 320,
              maxWidth: 450,
            ),
            child: ConfigPanel(api: api),
          ),
        ),
        const VerticalDivider(width: 1),
        Flexible(
          flex: 7,
          child: InfoPanel(api: api),
        ),
      ],
    );
  }
}