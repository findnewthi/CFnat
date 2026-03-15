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
              final aspectRatio = width / height;
              
              final isWide = width > 900;
              final isNarrow = width < 600;
              
              if (isWide) {
                return _buildWideLayout(api, width);
              }
              
              if (isNarrow || aspectRatio < 0.8) {
                return _buildNarrowLayout(api, width, height);
              }
              
              return _buildMediumLayout(api, width, height);
            },
          );
        },
      ),
    );
  }

  Widget _buildWideLayout(ApiService api, double width) {
    final configWidth = (width * 0.28).clamp(280.0, 360.0);
    
    return Row(
      children: [
        SizedBox(
          width: configWidth,
          child: ConfigPanel(api: api),
        ),
        const VerticalDivider(width: 1),
        Expanded(
          child: InfoPanel(api: api),
        ),
      ],
    );
  }

  Widget _buildNarrowLayout(ApiService api, double width, double height) {
    return Column(
      children: [
        Expanded(
          flex: 45,
          child: ConfigPanel(api: api, compact: true),
        ),
        const Divider(height: 1),
        Expanded(
          flex: 55,
          child: InfoPanel(api: api, forceVertical: true),
        ),
      ],
    );
  }

  Widget _buildMediumLayout(ApiService api, double width, double height) {
    final configWidth = (width * 0.35).clamp(260.0, 340.0);
    
    return Row(
      children: [
        SizedBox(
          width: configWidth,
          child: ConfigPanel(api: api),
        ),
        const VerticalDivider(width: 1),
        Expanded(
          child: InfoPanel(api: api),
        ),
      ],
    );
  }
}
