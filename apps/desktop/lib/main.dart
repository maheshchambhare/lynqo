import 'dart:async';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:window_manager/window_manager.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:hotkey_manager/hotkey_manager.dart';
import 'package:launch_at_startup/launch_at_startup.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'core/server_manager.dart';
import 'core/providers.dart';
import 'core/ws_client.dart';
import 'models.dart';
import 'ui/screens.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Initialize Launch at Startup
  try {
    PackageInfo packageInfo = await PackageInfo.fromPlatform();
    launchAtStartup.setup(
      appName: packageInfo.appName,
      appPath: Platform.resolvedExecutable,
      packageName: 'com.lynqo.lynqo',
    );
    await launchAtStartup.enable();
  } catch (e) {
    debugPrint("Failed to set up launch at startup: $e");
  }

  // Initialize Window Manager
  await windowManager.ensureInitialized();
  WindowOptions windowOptions = const WindowOptions(
    size: Size(960, 640),
    minimumSize: Size(850, 550),
    center: true,
    title: 'lynqo',
    titleBarStyle: TitleBarStyle.hidden, // Frameless window
  );
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    // Temporarily show the window invisibly to let the OS initialize the window structure, then hide it.
    // This resolves issues where background event dispatchers aren't registered by macOS.
    await windowManager.setOpacity(0.0);
    await windowManager.show();
    await windowManager.hide();
    await windowManager.setOpacity(1.0);
    await windowManager.setSkipTaskbar(false);
  });

  // Start the Rust sidecar server
  ServerManager().start();

  runApp(
    const ProviderScope(
      child: MyApp(),
    ),
  );
}

class MyApp extends ConsumerStatefulWidget {
  const MyApp({super.key});

  @override
  ConsumerState<MyApp> createState() => _MyAppState();
}

class _MyAppState extends ConsumerState<MyApp> with TrayListener, WindowListener {
  late WsClient _wsClient;
  Timer? _syncTimer;
  bool _isPopupOpen = false;
  String _lastClipboardText = '';
  DateTime _lastShownTime = DateTime.fromMillisecondsSinceEpoch(0);

  @override
  void initState() {
    super.initState();
    // Register system tray listener
    trayManager.addListener(this);
    _initSystemTray();

    // Register window listener
    windowManager.addListener(this);

    // Initialize hotkeys
    _initHotKeys();

    // Start local Dart clipboard watcher
    _startLocalClipboardWatcher();

    // Start WebSocket listener and trigger initial API sync
    _wsClient = ref.read(wsClientProvider);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _wsClient.connect();
      _syncData();
    });
  }

  Future<void> _initSystemTray() async {
    await trayManager.setIcon(
      'assets/tray_icon.png',
    );
    final Menu menu = Menu(
      items: [
        MenuItem(
          key: 'show_window',
          label: 'Show lynqo',
        ),
        MenuItem.separator(),
        MenuItem(
          key: 'exit_app',
          label: 'Quit',
        ),
      ],
    );
    await trayManager.setContextMenu(menu);
  }

  static const _windowStyleChannel = MethodChannel('lynqo/window_style');

  Future<void> _initHotKeys() async {
    await hotKeyManager.unregisterAll();
    HotKey hotKey = HotKey(
      key: PhysicalKeyboardKey.keyB, // Option + B
      modifiers: [HotKeyModifier.alt],
      scope: HotKeyScope.system,
    );
    try {
      await hotKeyManager.register(
        hotKey,
        keyDownHandler: (hotKey) async {
          debugPrint("Hotkey Option+B pressed! _isPopupOpen: $_isPopupOpen");
          if (_isPopupOpen) {
            _isPopupOpen = false;
            await windowManager.hide();
            debugPrint("Window hidden via hotkey toggle.");
          } else {
            _isPopupOpen = true;
            _lastShownTime = DateTime.now();
            ref.read(windowModeProvider.notifier).state = WindowMode.popup;
            await switchToPopupMode();
            await windowManager.show();
            await windowManager.focus();
            debugPrint("Window shown and focused via hotkey toggle.");
          }
        },
      );
      debugPrint("Hotkey Option+B registered successfully!");
    } catch (e) {
      debugPrint("Failed to register Hotkey Option+B: $e");
    }
  }

  void _startLocalClipboardWatcher() {
    Timer.periodic(const Duration(milliseconds: 500), (timer) async {
      if (!mounted) {
        timer.cancel();
        return;
      }
      try {
        final data = await Clipboard.getData(Clipboard.kTextPlain);
        if (data != null && data.text != null) {
          final text = data.text!;
          if (text.isNotEmpty && text != _lastClipboardText) {
            _lastClipboardText = text;

            final entry = ClipboardEntry(
              id: DateTime.now().millisecondsSinceEpoch.toString(),
              content: text,
              contentType: 'text/plain',
              source: 'desktop',
              createdAt: DateTime.now().millisecondsSinceEpoch,
              isFavorite: false,
              category: 'text',
              hash: text.hashCode.toString(),
            );

            ref.read(clipboardHistoryProvider.notifier).addOrUpdate(entry);

            // Also push to Rust server if online
            final status = ref.read(connectionStatusProvider);
            if (status == ConnectionStatus.connected) {
              ref.read(apiServiceProvider).pushClipboard(text);
            }
          }
        }
      } catch (e) {
        // Safe catch if clipboard is busy or locked
      }
    });
  }

  Future<void> switchToNormalMode() async {
    try {
      await _windowStyleChannel.invokeMethod('setNormalStyle');
    } catch (e) {
      debugPrint("Error resetting window style: $e");
    }
    await windowManager.setAlwaysOnTop(false);
    await windowManager.setResizable(true);
    await windowManager.setMinimumSize(const Size(850, 550));
    await windowManager.setSize(const Size(960, 640));
    await windowManager.center();
  }

  Future<void> switchToPopupMode() async {
    try {
      await _windowStyleChannel.invokeMethod('setPopupStyle');
    } catch (e) {
      debugPrint("Error setting popup window style: $e");
    }
    await windowManager.setAlwaysOnTop(true);
    await windowManager.setResizable(false);
    await windowManager.setMinimumSize(const Size(100, 100));
    await windowManager.setSize(const Size(500, 380));
    await windowManager.center();
  }

  @override
  void onWindowBlur() async {
    final mode = ref.read(windowModeProvider);
    if (mode == WindowMode.popup) {
      if (DateTime.now().difference(_lastShownTime).inMilliseconds < 500) {
        debugPrint("Ignoring transient blur event during window presentation.");
        return;
      }
      _isPopupOpen = false;
      await windowManager.hide();
    }
  }

  Future<void> _syncData() async {
    // Try to fetch initial state
    await ref.read(apiServiceProvider).fetchAll();
    // Retry periodically if offline
    _syncTimer = Timer.periodic(const Duration(seconds: 3), (timer) async {
      if (!mounted) {
        timer.cancel();
        return;
      }
      final status = ref.read(connectionStatusProvider);
      if (status == ConnectionStatus.connected) {
        await ref.read(apiServiceProvider).fetchAll();
        timer.cancel();
      } else {
        await ref.read(apiServiceProvider).fetchAll();
      }
    });
  }

  @override
  void onTrayIconMouseDown() async {
    _isPopupOpen = false;
    ref.read(windowModeProvider.notifier).state = WindowMode.normal;
    await switchToNormalMode();
    await windowManager.show();
    await windowManager.focus();
  }

  @override
  void onTrayMenuItemClick(MenuItem menuItem) async {
    if (menuItem.key == 'show_window') {
      _isPopupOpen = false;
      ref.read(windowModeProvider.notifier).state = WindowMode.normal;
      await switchToNormalMode();
      await windowManager.show();
      await windowManager.focus();
    } else if (menuItem.key == 'exit_app') {
      ServerManager().stop();
      windowManager.destroy();
    }
  }

  @override
  void dispose() {
    _syncTimer?.cancel();
    trayManager.removeListener(this);
    windowManager.removeListener(this);
    hotKeyManager.unregisterAll();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'lynqo',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: bgBase,
      ),
      home: const MainLayout(),
    );
  }
}

