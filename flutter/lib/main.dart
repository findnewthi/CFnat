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
            seedColor: const Color(0xFF2D7DFF),
            brightness: Brightness.dark,
          ),
          useMaterial3: true,
          brightness: Brightness.dark,
          scaffoldBackgroundColor: const Color(0xFF0E1117),
          cardTheme: CardThemeData(
            color: const Color(0xFF161B22),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: const BorderSide(color: Color(0xFF2A2F3A)),
            ),
          ),
          dividerColor: const Color(0xFF2A2F3A),
          appBarTheme: const AppBarTheme(
            backgroundColor: Color(0xFF121721),
            surfaceTintColor: Colors.transparent,
            elevation: 0,
          ),
        ),
        themeMode: ThemeMode.dark,
        home: const MainScreen(),
      ),
    );
  }
}