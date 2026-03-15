import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'screens/main_screen.dart';
import 'services/api_service.dart';

void main() {
  runApp(const CFnatApp());
}

class CFnatApp extends StatelessWidget {
  const CFnatApp({super.key});

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider(
      create: (_) => ApiService(),
      child: MaterialApp(
        title: 'CFnat Manager',
        debugShowCheckedModeBanner: false,
        theme: ThemeData(
          colorScheme: ColorScheme.fromSeed(
            seedColor: Colors.blue,
            brightness: Brightness.dark,
          ),
          useMaterial3: true,
          brightness: Brightness.dark,
        ),
        themeMode: ThemeMode.dark,
        home: const MainScreen(),
      ),
    );
  }
}
