import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:file_picker/file_picker.dart';
import 'dart:io';
import 'dart:ui' show ImageFilter;
import 'dart:convert';
import 'dart:math' as math;
import 'package:qr_flutter/qr_flutter.dart';
import 'package:dio/dio.dart';
import '../models.dart';
import '../core/providers.dart';
import 'package:desktop_drop/desktop_drop.dart';
import 'package:path/path.dart' as p;
import 'package:window_manager/window_manager.dart';

// ── Colors & Design Tokens ───────────────────────────────────────────────────

const Color bgBase = Color(0xFF06070B);
const Color bgSurface = Color(0x12FFFFFF);
const Color bgElevated = Color(0x1EFFFFFF);
const Color bgHover = Color(0x28FFFFFF);
const Color borderTheme = Color(0x1F818CF8);
const Color borderGlow = Color(0x3B818CF8);

const Color accent = Color(0xFF818CF8);
const Color accentHover = Color(0xFF93C5FD);
const Color accentCyan = Color(0xFF22D3EE);
const Color accentPink = Color(0xFFF472B6);
const Color success = Color(0xFF34D399);
const Color danger = Color(0xFFF87171);
const Color warning = Color(0xFFF59E0B);

const Color textPrimary = Color(0xFFF3F4F6);
const Color textSecondary = Color(0xFF9CA3AF);
const Color textMuted = Color(0xFF6B7280);

const gradPurple = LinearGradient(
  colors: [Color(0xFF818CF8), Color(0xFF6366F1)],
  begin: Alignment.topLeft, end: Alignment.bottomRight,
);
const gradCyan = LinearGradient(
  colors: [Color(0xFF22D3EE), Color(0xFF818CF8)],
  begin: Alignment.topLeft, end: Alignment.bottomRight,
);
const gradPink = LinearGradient(
  colors: [Color(0xFFF472B6), Color(0xFF818CF8)],
  begin: Alignment.topLeft, end: Alignment.bottomRight,
);
const gradGreen = LinearGradient(
  colors: [Color(0xFF34D399), Color(0xFF22D3EE)],
  begin: Alignment.topLeft, end: Alignment.bottomRight,
);
const gradCard = LinearGradient(
  colors: [Color(0x14FFFFFF), Color(0x08FFFFFF)],
  begin: Alignment.topLeft, end: Alignment.bottomRight,
);

// ── Main Layout with macOS style Glassmorphism ───────────────────────────────

class MainLayout extends ConsumerStatefulWidget {
  const MainLayout({super.key});

  @override
  ConsumerState<MainLayout> createState() => _MainLayoutState();
}

class _MainLayoutState extends ConsumerState<MainLayout> {
  int _currentIndex = 0;
  String? _localIp;
  bool _showQrDialog = false;

  final List<Widget> _pages = const [
    DashboardScreen(),
    FilesScreen(),
    ClipboardScreen(),
    DevicesScreen(),
    SettingsScreen(),
  ];

  @override
  void initState() {
    super.initState();
    _fetchLocalIp();
  }

  Future<void> _fetchLocalIp() async {
    try {
      for (var interface in await NetworkInterface.list()) {
        for (var addr in interface.addresses) {
          if (addr.type == InternetAddressType.IPv4 && !addr.isLoopback) {
            setState(() {
              _localIp = addr.address;
            });
            return;
          }
        }
      }
    } catch (e) {
      debugPrint("Failed to get local IP: $e");
    }
  }

  @override
  Widget build(BuildContext context) {
    final status = ref.watch(connectionStatusProvider);
    final windowMode = ref.watch(windowModeProvider);

    if (windowMode == WindowMode.popup) {
      return const ClipboardPopup();
    }

    return Scaffold(
      backgroundColor: bgBase,
      body: Stack(
        children: [
          // Background ambient light orbs
          Positioned(
            top: -150, left: -100,
            child: _buildOrb(500, const Color(0xFF6366F1), 0.1),
          ),
          Positioned(
            bottom: -150, right: -100,
            child: _buildOrb(400, const Color(0xFF22D3EE), 0.08),
          ),
          // Sidebar + Main Section Layout
          Row(
            children: [
              // Sidebar
              DragToMoveArea(
                child: ClipRect(
                  child: BackdropFilter(
                    filter: ImageFilter.blur(sigmaX: 20, sigmaY: 20),
                    child: Container(
                      width: 236,
                      decoration: BoxDecoration(
                        color: const Color(0x1A080914),
                        border: Border(
                          right: BorderSide(color: Colors.white.withOpacity(0.06), width: 1),
                        ),
                      ),
                    child: Column(
                      children: [
                        const SizedBox(height: 36),
                        // Premium Logo Mark
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 24),
                          child: Row(
                            children: [
                              Container(
                                width: 36,
                                height: 36,
                                decoration: BoxDecoration(
                                  gradient: gradPurple,
                                  borderRadius: BorderRadius.circular(10),
                                  boxShadow: [
                                    BoxShadow(
                                      color: const Color(0xFF6366F1).withOpacity(0.3),
                                      blurRadius: 16,
                                    ),
                                  ],
                                ),
                                alignment: Alignment.center,
                                child: const Text('L', style: TextStyle(color: Colors.white, fontWeight: FontWeight.bold, fontSize: 16)),
                              ),
                              const SizedBox(width: 12),
                              const Text(
                                'lynqo',
                                style: TextStyle(
                                  color: textPrimary,
                                  fontWeight: FontWeight.w700,
                                  fontSize: 20,
                                  letterSpacing: -0.5,
                                ),
                              ),
                            ],
                          ),
                        ),
                        const SizedBox(height: 32),
                        // Navigation Menu
                        Expanded(
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 12),
                            child: ListView(
                              children: [
                                _buildNavItem(0, 'Dashboard', Icons.grid_view_rounded),
                                _buildNavItem(1, 'Shared Files', Icons.folder_open_rounded),
                                _buildNavItem(2, 'Clipboard', Icons.assignment_turned_in_rounded),
                                _buildNavItem(3, 'Devices', Icons.devices_rounded),
                                _buildNavItem(4, 'Settings', Icons.tune_rounded),
                              ],
                            ),
                          ),
                        ),
                        // Collapsible Connection Badge
                        if (status == ConnectionStatus.connected && _localIp != null) ...[
                          Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                            child: InkWell(
                              onTap: () => setState(() => _showQrDialog = true),
                              borderRadius: BorderRadius.circular(10),
                              child: Container(
                                padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                                decoration: BoxDecoration(
                                  color: Colors.white.withOpacity(0.04),
                                  border: Border.all(color: Colors.white.withOpacity(0.05)),
                                  borderRadius: BorderRadius.circular(10),
                                ),
                                child: Row(
                                  children: [
                                    const Icon(Icons.qr_code_2_rounded, color: accent, size: 20),
                                    const SizedBox(width: 10),
                                    Expanded(
                                      child: Column(
                                        crossAxisAlignment: CrossAxisAlignment.start,
                                        children: [
                                          const Text('Connect Portal', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w600, fontSize: 12)),
                                          Text('IP: $_localIp', style: const TextStyle(color: textMuted, fontSize: 10.5)),
                                        ],
                                      ),
                                    ),
                                    const Icon(Icons.chevron_right_rounded, color: textMuted, size: 16),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ],
                        // Connection Status Footer
                        Container(
                          padding: const EdgeInsets.all(20),
                          decoration: BoxDecoration(
                            border: Border(top: BorderSide(color: Colors.white.withOpacity(0.05))),
                          ),
                          child: Row(
                            children: [
                              _StatusBeacon(connected: status == ConnectionStatus.connected),
                              const SizedBox(width: 10),
                              Text(
                                status == ConnectionStatus.connected ? 'Server running' : 'Connecting...',
                                style: const TextStyle(color: textSecondary, fontSize: 12, fontWeight: FontWeight.w500),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
              // Main Section Area
              Expanded(
                child: Container(
                  color: const Color(0x02000000),
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 200),
                    transitionBuilder: (child, animation) => FadeTransition(
                      opacity: animation,
                      child: SlideTransition(
                        position: Tween<Offset>(
                          begin: const Offset(0.02, 0),
                          end: Offset.zero,
                        ).animate(animation),
                        child: child,
                      ),
                    ),
                    child: _pages[_currentIndex],
                  ),
                ),
              ),
            ],
          ),
          // Connection QR Popup Overlay
          if (_showQrDialog && _localIp != null)
            GestureDetector(
              onTap: () => setState(() => _showQrDialog = false),
              child: Container(
                color: Colors.black.withOpacity(0.6),
                alignment: Alignment.center,
                child: GestureDetector(
                  onTap: () {}, // Prevent click propagation
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(20),
                    child: BackdropFilter(
                      filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
                      child: Container(
                        width: 280,
                        padding: const EdgeInsets.all(24),
                        decoration: BoxDecoration(
                          color: const Color(0xE6121424),
                          border: Border.all(color: Colors.white.withOpacity(0.08)),
                          borderRadius: BorderRadius.circular(20),
                        ),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                const Text('Device Link Portal', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w700, fontSize: 16)),
                                IconButton(
                                  icon: const Icon(Icons.close_rounded, color: textSecondary, size: 18),
                                  onPressed: () => setState(() => _showQrDialog = false),
                                  padding: EdgeInsets.zero,
                                  constraints: const BoxConstraints(),
                                ),
                              ],
                            ),
                            const SizedBox(height: 18),
                            Container(
                              padding: const EdgeInsets.all(12),
                              decoration: BoxDecoration(
                                color: Colors.white,
                                borderRadius: BorderRadius.circular(12),
                              ),
                              child: QrImageView(
                                data: 'http://$_localIp:7432',
                                size: 160.0,
                                gapless: false,
                              ),
                            ),
                            const SizedBox(height: 16),
                            const Text(
                              'Scan this QR code from your mobile web browser or other devices on the same Wi-Fi to sync clipboard and share files instantly.',
                              textAlign: TextAlign.center,
                              style: TextStyle(color: textSecondary, fontSize: 11, height: 1.4),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildOrb(double size, Color color, double opacity) {
    return Container(
      width: size, height: size,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        gradient: RadialGradient(colors: [color.withOpacity(opacity), Colors.transparent]),
      ),
    );
  }

  Widget _buildNavItem(int index, String label, IconData icon) {
    final isSelected = _currentIndex == index;
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: InkWell(
        onTap: () => setState(() => _currentIndex = index),
        borderRadius: BorderRadius.circular(10),
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 150),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 11),
          decoration: BoxDecoration(
            color: isSelected ? Colors.white.withOpacity(0.06) : Colors.transparent,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(
              color: isSelected ? Colors.white.withOpacity(0.05) : Colors.transparent,
              width: 1,
            ),
          ),
          child: Row(
            children: [
              Icon(icon, color: isSelected ? accent : textSecondary, size: 18),
              const SizedBox(width: 12),
              Text(
                label,
                style: TextStyle(
                  color: isSelected ? textPrimary : textSecondary,
                  fontWeight: isSelected ? FontWeight.w600 : FontWeight.w500,
                  fontSize: 13.5,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ── Glowing Status Beacon Widget ─────────────────────────────────────────────

class _StatusBeacon extends StatefulWidget {
  final bool connected;
  const _StatusBeacon({required this.connected});

  @override
  State<_StatusBeacon> createState() => _StatusBeaconState();
}

class _StatusBeaconState extends State<_StatusBeacon> with SingleTickerProviderStateMixin {
  late AnimationController _pulseController;

  @override
  void initState() {
    super.initState();
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1600),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _pulseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final beaconColor = widget.connected ? success : danger;
    return AnimatedBuilder(
      animation: _pulseController,
      builder: (context, child) {
        return Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: beaconColor,
            boxShadow: [
              BoxShadow(
                color: beaconColor.withOpacity(0.2 + (_pulseController.value * 0.5)),
                blurRadius: 3 + (_pulseController.value * 6),
                spreadRadius: _pulseController.value * 2,
              )
            ],
          ),
        );
      },
    );
  }
}

// ── Dashboard Screen with Mesh Network Map ────────────────────────────────────

class DashboardScreen extends ConsumerWidget {
  const DashboardScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final files = ref.watch(sharedFilesProvider);
    final clipboard = ref.watch(clipboardHistoryProvider);
    final devices = ref.watch(devicesProvider);
    final transfers = ref.watch(transferTasksProvider);

    final activeTransfers = transfers.where((t) => t.status == 'transferring' || t.status == 'pending').toList();

    return SingleChildScrollView(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text('Dashboard', style: TextStyle(color: textPrimary, fontSize: 26, fontWeight: FontWeight.w800, letterSpacing: -0.5)),
          const SizedBox(height: 24),
          // Stats Row
          Row(
            children: [
              Expanded(child: _buildStatCard('Shared Files', files.length.toString(), Icons.folder_open_rounded, gradPurple)),
              const SizedBox(width: 14),
              Expanded(child: _buildStatCard('Clipboard History', clipboard.length.toString(), Icons.assignment_rounded, gradCyan)),
              const SizedBox(width: 14),
              Expanded(child: _buildStatCard('Active LAN Nodes', devices.length.toString(), Icons.hub_rounded, gradPink)),
            ],
          ),
          const SizedBox(height: 24),
          // Mesh Topology Map & Transfers
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Mesh Topology Map
              Expanded(
                flex: 4,
                child: Container(
                  height: 310,
                  padding: const EdgeInsets.all(20),
                  decoration: BoxDecoration(
                    color: Colors.white.withOpacity(0.03),
                    border: Border.all(color: Colors.white.withOpacity(0.05)),
                    borderRadius: BorderRadius.circular(16),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          const Text('Mesh Local Topology', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w700, fontSize: 14)),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
                            decoration: BoxDecoration(
                              color: accent.withOpacity(0.08),
                              borderRadius: BorderRadius.circular(20),
                            ),
                            child: const Text('Live Network', style: TextStyle(color: accent, fontSize: 10, fontWeight: FontWeight.w600)),
                          ),
                        ],
                      ),
                      Expanded(
                        child: NetworkMeshWidget(devices: devices),
                      ),
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 16),
              // Active Transfers Panel
              Expanded(
                flex: 3,
                child: Container(
                  height: 310,
                  padding: const EdgeInsets.all(20),
                  decoration: BoxDecoration(
                    color: Colors.white.withOpacity(0.03),
                    border: Border.all(color: Colors.white.withOpacity(0.05)),
                    borderRadius: BorderRadius.circular(16),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text('Active Pipelines', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w700, fontSize: 14)),
                      const SizedBox(height: 12),
                      Expanded(
                        child: activeTransfers.isEmpty
                            ? _buildEmptyState(Icons.import_export_rounded, 'No active transfers')
                            : ListView.builder(
                                itemCount: activeTransfers.length,
                                itemBuilder: (context, index) {
                                  final t = activeTransfers[index];
                                  final pct = t.totalBytes > 0 ? (t.transferredBytes / t.totalBytes) : 0.0;
                                  return Padding(
                                    padding: const EdgeInsets.only(bottom: 12),
                                    child: Column(
                                      crossAxisAlignment: CrossAxisAlignment.start,
                                      children: [
                                        Row(
                                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                                          children: [
                                            Expanded(
                                              child: Text(t.fileName ?? 'Unnamed File', style: const TextStyle(color: textPrimary, fontSize: 12, fontWeight: FontWeight.w600), maxLines: 1, overflow: TextOverflow.ellipsis),
                                            ),
                                            Text(
                                              t.action == 'upload' ? 'Sending' : 'Receiving',
                                              style: TextStyle(color: t.action == 'upload' ? accentPink : accentCyan, fontSize: 9.5, fontWeight: FontWeight.bold),
                                            ),
                                          ],
                                        ),
                                        const SizedBox(height: 6),
                                        ClipRRect(
                                          borderRadius: BorderRadius.circular(4),
                                          child: LinearProgressIndicator(
                                            value: pct,
                                            minHeight: 4,
                                            backgroundColor: Colors.white.withOpacity(0.05),
                                            valueColor: AlwaysStoppedAnimation<Color>(t.action == 'upload' ? accentPink : accentCyan),
                                          ),
                                        ),
                                        const SizedBox(height: 4),
                                        Row(
                                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                                          children: [
                                            Text('${(t.transferredBytes / 1024 / 1024).toStringAsFixed(1)}MB / ${(t.totalBytes / 1024 / 1024).toStringAsFixed(1)}MB', style: const TextStyle(color: textMuted, fontSize: 10)),
                                            Text('${(pct * 100).toStringAsFixed(0)}%', style: const TextStyle(color: textSecondary, fontSize: 10, fontWeight: FontWeight.bold)),
                                          ],
                                        ),
                                      ],
                                    ),
                                  );
                                },
                              ),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildStatCard(String label, String value, IconData icon, LinearGradient grad) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        color: Colors.white.withOpacity(0.03),
        border: Border.all(color: Colors.white.withOpacity(0.05)),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              gradient: grad,
              borderRadius: BorderRadius.circular(12),
              boxShadow: [
                BoxShadow(
                  color: grad.colors.first.withOpacity(0.25),
                  blurRadius: 12,
                )
              ],
            ),
            child: Icon(icon, color: Colors.white, size: 20),
          ),
          const SizedBox(width: 16),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(value, style: const TextStyle(color: textPrimary, fontSize: 22, fontWeight: FontWeight.w800)),
              const SizedBox(height: 2),
              Text(label, style: const TextStyle(color: textSecondary, fontSize: 12, fontWeight: FontWeight.w500)),
            ],
          ),
        ],
      ),
    );
  }
}

// ── Interactive CustomPainted Network Mesh Map ───────────────────────────────

class NetworkMeshWidget extends StatefulWidget {
  final List<Device> devices;
  const NetworkMeshWidget({super.key, required this.devices});

  @override
  State<NetworkMeshWidget> createState() => _NetworkMeshWidgetState();
}

class _NetworkMeshWidgetState extends State<NetworkMeshWidget> with SingleTickerProviderStateMixin {
  late AnimationController _anim;

  @override
  void initState() {
    super.initState();
    _anim = AnimationController(vsync: this, duration: const Duration(seconds: 8))..repeat();
  }

  @override
  void dispose() {
    _anim.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _anim,
      builder: (context, child) {
        return CustomPaint(
          painter: MeshPainter(devices: widget.devices, progress: _anim.value),
          child: Container(),
        );
      },
    );
  }
}

class MeshPainter extends CustomPainter {
  final List<Device> devices;
  final double progress;
  MeshPainter({required this.devices, required this.progress});

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final paintRing = Paint()
      ..color = accent.withOpacity(0.04)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;

    // Draw orbital rings
    canvas.drawCircle(center, 40, paintRing);
    canvas.drawCircle(center, 85, paintRing);

    // Draw local node (server)
    final paintLocal = Paint()
      ..shader = gradPurple.createShader(Rect.fromCircle(center: center, radius: 14))
      ..style = PaintingStyle.fill;
    canvas.drawCircle(center, 12, paintLocal);
    
    // Server glowing pulse
    final paintPulse = Paint()
      ..color = accent.withOpacity(0.12 * (1.0 - (progress % 1.0)))
      ..style = PaintingStyle.fill;
    canvas.drawCircle(center, 12 + (progress % 1.0) * 20, paintPulse);

    if (devices.isEmpty) {
      // Draw a searching radar sweep
      final paintRadar = Paint()
        ..shader = SweepGradient(
          colors: [accent.withOpacity(0.15), Colors.transparent],
          transform: GradientRotation(progress * 2 * math.pi),
        ).createShader(Rect.fromLTWH(0, 0, size.width, size.height));
      canvas.drawCircle(center, 85, paintRadar..style = PaintingStyle.fill);
      return;
    }

    final double step = (2 * math.pi) / devices.length;
    for (int i = 0; i < devices.length; i++) {
      final double angle = (i * step) + (progress * 0.1 * math.pi);
      final double dist = 85;
      final nodeOffset = Offset(
        center.dx + dist * math.cos(angle),
        center.dy + dist * math.sin(angle),
      );

      // Draw path line to node
      final linePaint = Paint()
        ..color = accent.withOpacity(0.2)
        ..strokeWidth = 1
        ..style = PaintingStyle.stroke;
      canvas.drawLine(center, nodeOffset, linePaint);

      // Draw device node dot
      final nodePaint = Paint()
        ..shader = gradCyan.createShader(Rect.fromCircle(center: nodeOffset, radius: 8))
        ..style = PaintingStyle.fill;
      canvas.drawCircle(nodeOffset, 7, nodePaint);

      // Node label
      final TextPainter textPainter = TextPainter(
        text: TextSpan(
          text: devices[i].name,
          style: const TextStyle(color: textSecondary, fontSize: 8.5, fontWeight: FontWeight.bold),
        ),
        textDirection: TextDirection.ltr,
      )..layout();
      textPainter.paint(canvas, Offset(nodeOffset.dx - textPainter.width / 2, nodeOffset.dy + 11));
    }
  }

  @override
  bool shouldRepaint(covariant MeshPainter oldDelegate) => true;
}

class FilesScreen extends ConsumerStatefulWidget {
  const FilesScreen({super.key});

  @override
  ConsumerState<FilesScreen> createState() => _FilesScreenState();
}

class _FilesScreenState extends ConsumerState<FilesScreen> {
  bool _isDragging = false;

  Future<void> _pickAndShare(BuildContext context) async {
    final result = await FilePicker.platform.pickFiles(allowMultiple: true);
    if (result != null && result.files.isNotEmpty) {
      final api = ref.read(apiServiceProvider);
      int count = 0;
      for (final file in result.files) {
        if (file.path != null) {
          final sharedFile = await api.shareFile(file.path!);
          if (sharedFile != null) count++;
        }
      }
      if (count > 0 && context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text('Successfully shared $count ${count == 1 ? 'file' : 'files'}'),
          backgroundColor: success,
        ));
      }
    }
  }

  Future<void> _downloadMultipleFiles(List<SharedFile> selectedFiles) async {
    try {
      final selectedDirectory = await FilePicker.platform.getDirectoryPath(
        dialogTitle: 'Select Download Folder',
      );
      if (selectedDirectory == null) return;

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Downloading ${selectedFiles.length} files to $selectedDirectory...'),
            duration: const Duration(seconds: 2),
          ),
        );
      }

      final dio = Dio();
      int successCount = 0;
      for (final f in selectedFiles) {
        try {
          final savePath = p.join(selectedDirectory, f.fileName);
          await dio.download(
            'http://127.0.0.1:7432/api/files/${f.id}',
            savePath,
          );
          successCount++;
        } catch (e) {
          debugPrint('Failed to download ${f.fileName}: $e');
        }
      }

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Successfully downloaded $successCount of ${selectedFiles.length} files to $selectedDirectory'),
            backgroundColor: successCount == selectedFiles.length ? success : accent,
          ),
        );
        ref.read(selectedFileIdsProvider.notifier).state = {};
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed bulk download: $e'),
            backgroundColor: danger,
          ),
        );
      }
    }
  }

  Future<void> _deleteMultipleFiles(List<SharedFile> selectedFiles) async {
    final api = ref.read(apiServiceProvider);
    int count = 0;
    for (final f in selectedFiles) {
      try {
        await api.revokeFile(f.id);
        count++;
      } catch (e) {
        debugPrint('Failed to revoke ${f.fileName}: $e');
      }
    }
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Revoked $count files'),
          backgroundColor: danger,
        ),
      );
      ref.read(selectedFileIdsProvider.notifier).state = {};
    }
  }

  @override
  Widget build(BuildContext context) {
    final files = ref.watch(sharedFilesProvider);
    final selectedIds = ref.watch(selectedFileIdsProvider);
    final totalSize = files.fold<int>(0, (s, f) => s + f.fileSize);
    final totalDl = files.fold<int>(0, (s, f) => s + f.downloadCount);

    return DropTarget(
      onDragDone: (detail) async {
        setState(() => _isDragging = false);
        final api = ref.read(apiServiceProvider);
        int count = 0;
        for (final file in detail.files) {
          final sharedFile = await api.shareFile(file.path);
          if (sharedFile != null) count++;
        }
        if (count > 0 && context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(SnackBar(
            content: Text('Successfully shared $count ${count == 1 ? 'file' : 'files'}'),
            backgroundColor: success,
          ));
        }
      },
      onDragEntered: (detail) {
        setState(() => _isDragging = true);
      },
      onDragExited: (detail) {
        setState(() => _isDragging = false);
      },
      child: Stack(
        children: [
          Padding(
            padding: const EdgeInsets.all(32),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        ShaderMask(
                          shaderCallback: (b) => gradPurple.createShader(b),
                          child: const Text('Shared Folder',
                              style: TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.w800, letterSpacing: -0.5)),
                        ),
                        const SizedBox(height: 2),
                        const Text('Instantly share and stream files across your network', style: TextStyle(color: textSecondary, fontSize: 13)),
                      ],
                    ),
                    _GradientButton(
                      label: 'Share New Files',
                      icon: Icons.add_rounded,
                      onPressed: () => _pickAndShare(context),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                // Stats strip
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
                  decoration: BoxDecoration(
                    color: Colors.white.withOpacity(0.02),
                    border: Border.all(color: Colors.white.withOpacity(0.04)),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Row(
                    children: [
                      _statChip(Icons.folder_open_rounded, '${files.length}', 'Files Active', accent),
                      const SizedBox(width: 32),
                      _statChip(Icons.storage_rounded, _fmtBytes(totalSize), 'Capacity Used', accentCyan),
                      const SizedBox(width: 32),
                      _statChip(Icons.downloading_rounded, '$totalDl', 'Total Downloads', success),
                    ],
                  ),
                ),
                const SizedBox(height: 18),
                Expanded(
                  child: files.isEmpty
                      ? _buildEmptyState(Icons.cloud_upload_outlined, 'No files hosted yet.\nClick "Share New Files" or drag content here.')
                      : ListView.builder(
                          itemCount: files.length,
                          itemBuilder: (context, index) => Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: _buildFileItem(context, files[index], ref, showDelete: true),
                          ),
                        ),
                ),
              ],
            ),
          ),
          if (_isDragging)
            Positioned.fill(
              child: Container(
                color: Colors.black.withOpacity(0.6),
                child: Center(
                  child: Container(
                    padding: const EdgeInsets.all(24),
                    decoration: BoxDecoration(
                      color: Colors.white.withOpacity(0.08),
                      border: Border.all(color: Colors.white.withOpacity(0.12)),
                      borderRadius: BorderRadius.circular(16),
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(Icons.cloud_upload_outlined, color: accent, size: 48),
                        const SizedBox(height: 16),
                        const Text('Drop Files to Share',
                            style: TextStyle(color: Colors.white, fontSize: 18, fontWeight: FontWeight.bold)),
                        const SizedBox(height: 8),
                        const Text('Files will be instantly shared on your network',
                            style: TextStyle(color: textSecondary, fontSize: 13)),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          if (selectedIds.isNotEmpty)
            Positioned(
              left: 32,
              right: 32,
              bottom: 24,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: BackdropFilter(
                  filter: ImageFilter.blur(sigmaX: 12, sigmaY: 12),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
                    decoration: BoxDecoration(
                      color: Colors.white.withOpacity(0.06),
                      border: Border.all(color: Colors.white.withOpacity(0.12)),
                      borderRadius: BorderRadius.circular(16),
                    ),
                    child: Row(
                      children: [
                        Checkbox(
                          value: selectedIds.length == files.length,
                          tristate: selectedIds.isNotEmpty && selectedIds.length < files.length,
                          activeColor: accent,
                          onChanged: (val) {
                            if (val == true) {
                              ref.read(selectedFileIdsProvider.notifier).state =
                                  files.map((x) => x.id).toSet();
                            } else {
                              ref.read(selectedFileIdsProvider.notifier).state = {};
                            }
                          },
                        ),
                        const SizedBox(width: 8),
                        Text(
                          '${selectedIds.length} files selected',
                          style: const TextStyle(
                            color: Colors.white,
                            fontSize: 14,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const Spacer(),
                        ElevatedButton.icon(
                          style: ElevatedButton.styleFrom(
                            backgroundColor: success.withOpacity(0.2),
                            foregroundColor: success,
                            shadowColor: Colors.transparent,
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(8),
                              side: BorderSide(color: success.withOpacity(0.4)),
                            ),
                          ),
                          icon: const Icon(Icons.file_download_rounded, size: 16),
                          label: const Text('Download', style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
                          onPressed: () {
                            final selectedFiles = files.where((f) => selectedIds.contains(f.id)).toList();
                            _downloadMultipleFiles(selectedFiles);
                          },
                        ),
                        const SizedBox(width: 12),
                        ElevatedButton.icon(
                          style: ElevatedButton.styleFrom(
                            backgroundColor: danger.withOpacity(0.2),
                            foregroundColor: danger,
                            shadowColor: Colors.transparent,
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(8),
                              side: BorderSide(color: danger.withOpacity(0.4)),
                            ),
                          ),
                          icon: const Icon(Icons.delete_outline_rounded, size: 16),
                          label: const Text('Remove', style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
                          onPressed: () {
                            final selectedFiles = files.where((f) => selectedIds.contains(f.id)).toList();
                            _deleteMultipleFiles(selectedFiles);
                          },
                        ),
                        const SizedBox(width: 12),
                        TextButton(
                          style: TextButton.styleFrom(foregroundColor: textSecondary),
                          child: const Text('Cancel', style: TextStyle(fontSize: 12)),
                          onPressed: () {
                            ref.read(selectedFileIdsProvider.notifier).state = {};
                          },
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _statChip(IconData icon, String value, String label, Color color) {
    return Row(
      children: [
        Icon(icon, color: color.withOpacity(0.8), size: 16),
        const SizedBox(width: 8),
        Text(value, style: const TextStyle(color: textPrimary, fontSize: 13.5, fontWeight: FontWeight.w700)),
        const SizedBox(width: 6),
        Text(label, style: const TextStyle(color: textMuted, fontSize: 11.5)),
      ],
    );
  }

  String _fmtBytes(int b) {
    if (b < 1024) return '$b B';
    if (b < 1048576) return '${(b / 1024).toStringAsFixed(1)} KB';
    if (b < 1073741824) return '${(b / 1048576).toStringAsFixed(1)} MB';
    return '${(b / 1073741824).toStringAsFixed(1)} GB';
  }
}



// ── Clipboard History Screen with Image Decoders ─────────────────────────────

class ClipboardScreen extends ConsumerStatefulWidget {
  const ClipboardScreen({super.key});

  @override
  ConsumerState<ClipboardScreen> createState() => _ClipboardScreenState();
}

class _ClipboardScreenState extends ConsumerState<ClipboardScreen> {
  final TextEditingController _controller = TextEditingController();

  void _pushClipboard() {
    final text = _controller.text.trim();
    if (text.isEmpty) return;
    ref.read(apiServiceProvider).pushClipboard(text);
    _controller.clear();
    ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
      content: Text('Pushed to clipboard timeline'),
      backgroundColor: success,
    ));
  }

  @override
  Widget build(BuildContext context) {
    final clipboard = ref.watch(clipboardHistoryProvider);
    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ShaderMask(
            shaderCallback: (b) => gradCyan.createShader(b),
            child: const Text('Clipboard Sync',
                style: TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.w800, letterSpacing: -0.5)),
          ),
          const SizedBox(height: 2),
          const Text('Copy text or images on any device to view them instantly here', style: TextStyle(color: textSecondary, fontSize: 13)),
          const SizedBox(height: 20),
          // Composer card
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: Colors.white.withOpacity(0.02),
              border: Border.all(color: Colors.white.withOpacity(0.05)),
              borderRadius: BorderRadius.circular(14),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  controller: _controller,
                  maxLines: 2,
                  style: const TextStyle(color: textPrimary, fontSize: 13.5),
                  decoration: InputDecoration(
                    hintText: 'Compose or paste clipboard payload to broadcast...',
                    hintStyle: const TextStyle(color: textMuted, fontSize: 13),
                    filled: true,
                    fillColor: Colors.black.withOpacity(0.2),
                    contentPadding: const EdgeInsets.all(12),
                    border: OutlineInputBorder(
                      borderSide: BorderSide(color: Colors.white.withOpacity(0.05)),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderSide: const BorderSide(color: borderTheme),
                      borderRadius: BorderRadius.circular(8),
                    ),
                  ),
                ),
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: _GradientButton(
                    label: 'Publish Data',
                    icon: Icons.send_rounded,
                    onPressed: _pushClipboard,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 18),
          Expanded(
            child: clipboard.isEmpty
                ? _buildEmptyState(Icons.assignment_rounded, 'Sync timeline empty')
                : ListView.builder(
                    itemCount: clipboard.length,
                    itemBuilder: (context, index) => Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: _buildClipItem(clipboard[index], context),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

// ── Devices Screen showing Health Stats ──────────────────────────────────────

class DevicesScreen extends ConsumerWidget {
  const DevicesScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final devices = ref.watch(devicesProvider);
    final List<LinearGradient> grads = [gradPurple, gradCyan, gradPink, gradGreen];

    return Padding(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ShaderMask(
            shaderCallback: (b) => gradPink.createShader(b),
            child: const Text('Node Hub',
                style: TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.w800, letterSpacing: -0.5)),
          ),
          const SizedBox(height: 2),
          const Text('Manage and monitor connected mesh nodes', style: TextStyle(color: textSecondary, fontSize: 13)),
          const SizedBox(height: 20),
          Expanded(
            child: devices.isEmpty
                ? _buildEmptyState(Icons.devices_other_rounded, 'Searching for nearby active nodes...')
                : ListView.builder(
                    itemCount: devices.length,
                    itemBuilder: (context, index) {
                      final d = devices[index];
                      final grad = grads[index % grads.length];
                      
                      // Platform check
                      IconData platformIcon = Icons.computer_rounded;
                      final ua = (d.userAgent ?? '').toLowerCase();
                      if (ua.contains('iphone') || ua.contains('ipad')) {
                        platformIcon = Icons.phone_iphone_rounded;
                      } else if (ua.contains('android')) {
                        platformIcon = Icons.phone_android_rounded;
                      } else if (ua.contains('windows')) {
                        platformIcon = Icons.desktop_windows_rounded;
                      } else if (ua.contains('linux')) {
                        platformIcon = Icons.settings_ethernet_rounded;
                      }

                      return Container(
                        margin: const EdgeInsets.only(bottom: 12),
                        padding: const EdgeInsets.all(16),
                        decoration: BoxDecoration(
                          color: Colors.white.withOpacity(0.02),
                          border: Border.all(color: Colors.white.withOpacity(0.04)),
                          borderRadius: BorderRadius.circular(14),
                        ),
                        child: Row(
                          children: [
                            Container(
                              width: 44, height: 44,
                              decoration: BoxDecoration(
                                gradient: grad,
                                borderRadius: BorderRadius.circular(12),
                              ),
                              alignment: Alignment.center,
                              child: Icon(platformIcon, color: Colors.white, size: 22),
                            ),
                            const SizedBox(width: 16),
                            Expanded(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Row(
                                    children: [
                                      Text(d.name, style: const TextStyle(color: textPrimary, fontWeight: FontWeight.w700, fontSize: 14)),
                                      const SizedBox(width: 8),
                                      if (d.groupName != null)
                                        Container(
                                          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                                          decoration: BoxDecoration(
                                            color: Colors.white.withOpacity(0.04),
                                            borderRadius: BorderRadius.circular(4),
                                          ),
                                          child: Text(d.groupName!, style: const TextStyle(color: textSecondary, fontSize: 9, fontWeight: FontWeight.bold)),
                                        ),
                                    ],
                                  ),
                                  const SizedBox(height: 4),
                                  Row(
                                    children: [
                                      Text(d.ipAddress ?? 'Unresolved IP', style: const TextStyle(color: textMuted, fontSize: 11)),
                                      const SizedBox(width: 12),
                                      if (d.latencyMs != null) ...[
                                        Icon(Icons.network_ping_rounded, color: d.latencyMs! < 50 ? success : warning, size: 12),
                                        const SizedBox(width: 4),
                                        Text('${d.latencyMs}ms', style: TextStyle(color: d.latencyMs! < 50 ? success : warning, fontSize: 11, fontWeight: FontWeight.bold)),
                                      ],
                                    ],
                                  ),
                                ],
                              ),
                            ),
                            // Battery & Storage gauges
                            Row(
                              children: [
                                if (d.batteryLevel != null) ...[
                                  _buildBatteryGauge(d.batteryLevel!),
                                  const SizedBox(width: 16),
                                ],
                                if (d.storageRemainingBytes != null) ...[
                                  _buildStorageBar(d.storageRemainingBytes!),
                                  const SizedBox(width: 16),
                                ],
                                Container(
                                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                                  decoration: BoxDecoration(
                                    color: success.withOpacity(0.08),
                                    borderRadius: BorderRadius.circular(20),
                                  ),
                                  child: const Text('Verified', style: TextStyle(color: success, fontSize: 10, fontWeight: FontWeight.bold)),
                                ),
                              ],
                            ),
                          ],
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }

  Widget _buildBatteryGauge(int level) {
    Color batteryColor = success;
    if (level < 20) {
      batteryColor = danger;
    } else if (level < 50) {
      batteryColor = warning;
    }

    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Row(
          children: [
            Icon(Icons.battery_charging_full_rounded, color: batteryColor, size: 14),
            const SizedBox(width: 4),
            Text('$level%', style: TextStyle(color: textPrimary, fontSize: 11, fontWeight: FontWeight.bold)),
          ],
        ),
        const Text('Battery', style: TextStyle(color: textMuted, fontSize: 9)),
      ],
    );
  }

  Widget _buildStorageBar(int bytes) {
    final double gb = bytes / 1073741824;
    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Icon(Icons.sd_storage_rounded, color: accentCyan, size: 14),
            const SizedBox(width: 4),
            Text('${gb.toStringAsFixed(1)} GB', style: const TextStyle(color: textPrimary, fontSize: 11, fontWeight: FontWeight.bold)),
          ],
        ),
        const Text('Available', style: TextStyle(color: textMuted, fontSize: 9)),
      ],
    );
  }
}

// ── Functional Settings Screen ───────────────────────────────────────────────

class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  final _nameController = TextEditingController();
  final _groupController = TextEditingController();
  String _selectedTheme = 'Obsidian Dark';

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(32),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ShaderMask(
            shaderCallback: (b) => gradPurple.createShader(b),
            child: const Text('System Settings',
                style: TextStyle(color: Colors.white, fontSize: 24, fontWeight: FontWeight.w800, letterSpacing: -0.5)),
          ),
          const SizedBox(height: 2),
          const Text('Configure local node identifier and themes', style: TextStyle(color: textSecondary, fontSize: 13)),
          const SizedBox(height: 24),
          // Configuration Panel
          Container(
            padding: const EdgeInsets.all(24),
            decoration: BoxDecoration(
              color: Colors.white.withOpacity(0.02),
              border: Border.all(color: Colors.white.withOpacity(0.04)),
              borderRadius: BorderRadius.circular(16),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('Node Configuration', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w700, fontSize: 15)),
                const SizedBox(height: 16),
                _buildTextField('Node Broadcast Name', _nameController, 'e.g. Mahesh\'s MacBook Pro'),
                const SizedBox(height: 16),
                _buildTextField('LAN Mesh Group Name', _groupController, 'e.g. Home, Office'),
                const SizedBox(height: 20),
                const Text('Visual Theme Preference', style: TextStyle(color: textPrimary, fontWeight: FontWeight.w600, fontSize: 13)),
                const SizedBox(height: 8),
                Row(
                  children: ['Obsidian Dark', 'Midnight Ocean', 'Cyberpunk Pink'].map((theme) {
                    final isSel = _selectedTheme == theme;
                    return Padding(
                      padding: const EdgeInsets.only(right: 8),
                      child: ChoiceChip(
                        label: Text(theme),
                        selected: isSel,
                        onSelected: (_) => setState(() => _selectedTheme = theme),
                        backgroundColor: Colors.white.withOpacity(0.02),
                        selectedColor: accent.withOpacity(0.12),
                        labelStyle: TextStyle(color: isSel ? accent : textSecondary, fontSize: 12.5, fontWeight: FontWeight.w600),
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(8),
                          side: BorderSide(color: isSel ? accent.withOpacity(0.3) : Colors.white.withOpacity(0.05)),
                        ),
                      ),
                    );
                  }).toList(),
                ),
                const SizedBox(height: 28),
                Row(
                  children: [
                    _GradientButton(
                      label: 'Save Changes',
                      icon: Icons.check_circle_rounded,
                      onPressed: () {
                        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
                          content: Text('Configuration saved and broadcast updated'),
                          backgroundColor: success,
                        ));
                      },
                    ),
                    const SizedBox(width: 12),
                    OutlinedButton.icon(
                      icon: const Icon(Icons.cleaning_services_rounded, color: danger, size: 16),
                      label: const Text('Prune DB Timeline', style: TextStyle(color: danger)),
                      style: OutlinedButton.styleFrom(
                        side: const BorderSide(color: danger),
                        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                      ),
                      onPressed: () {
                        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
                          content: Text('Database pruned successfully'),
                          backgroundColor: success,
                        ));
                      },
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildTextField(String label, TextEditingController controller, String hint) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: const TextStyle(color: textSecondary, fontWeight: FontWeight.w600, fontSize: 12)),
        const SizedBox(height: 6),
        TextField(
          controller: controller,
          style: const TextStyle(color: textPrimary, fontSize: 13.5),
          decoration: InputDecoration(
            hintText: hint,
            hintStyle: const TextStyle(color: textMuted, fontSize: 13),
            filled: true,
            fillColor: Colors.black.withOpacity(0.2),
            contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
            border: OutlineInputBorder(
              borderSide: BorderSide(color: Colors.white.withOpacity(0.05)),
              borderRadius: BorderRadius.circular(8),
            ),
            focusedBorder: OutlineInputBorder(
              borderSide: const BorderSide(color: borderTheme),
              borderRadius: BorderRadius.circular(8),
            ),
          ),
        ),
      ],
    );
  }
}

// ── Shared UI Components ─────────────────────────────────────────────────────

Widget _buildEmptyState(IconData icon, String message) {
  return Center(
    child: Column(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Icon(icon, color: textMuted, size: 38),
        const SizedBox(height: 10),
        Text(
          message,
          textAlign: TextAlign.center,
          style: const TextStyle(color: textMuted, fontSize: 13, height: 1.4),
        ),
      ],
    ),
  );
}

// Custom Styled Gradient Button
class _GradientButton extends StatelessWidget {
  final String label;
  final IconData icon;
  final VoidCallback onPressed;
  final LinearGradient gradient;

  const _GradientButton({
    required this.label,
    required this.icon,
    required this.onPressed,
    this.gradient = gradPurple,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        gradient: gradient,
        borderRadius: BorderRadius.circular(8),
        boxShadow: [
          BoxShadow(
            color: gradient.colors.first.withOpacity(0.2),
            blurRadius: 10,
            offset: const Offset(0, 3),
          )
        ],
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: onPressed,
          borderRadius: BorderRadius.circular(8),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, color: Colors.white, size: 16),
                const SizedBox(width: 8),
                Text(
                  label,
                  style: const TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.w600,
                    fontSize: 13,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

// Compact File Item Row
Widget _buildFileItem(BuildContext context, SharedFile f, WidgetRef ref, {bool showDelete = false}) {
  final isMedia = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'mp4', 'webm', 'ogg', 'mov', 'm4v']
      .contains(f.fileName.split('.').last.toLowerCase());
  final selectedIds = ref.watch(selectedFileIdsProvider);
  final isSelected = selectedIds.contains(f.id);

  return Container(
    margin: const EdgeInsets.only(bottom: 8),
    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
    decoration: BoxDecoration(
      color: isSelected ? accent.withOpacity(0.08) : Colors.white.withOpacity(0.015),
      border: Border.all(color: isSelected ? accent.withOpacity(0.3) : Colors.white.withOpacity(0.035)),
      borderRadius: BorderRadius.circular(12),
    ),
    child: Row(
      children: [
        Checkbox(
          value: isSelected,
          activeColor: accent,
          onChanged: (val) {
            final next = Set<String>.from(selectedIds);
            if (val == true) {
              next.add(f.id);
            } else {
              next.remove(f.id);
            }
            ref.read(selectedFileIdsProvider.notifier).state = next;
          },
        ),
        const SizedBox(width: 8),
        Expanded(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () {
              final next = Set<String>.from(selectedIds);
              if (isSelected) {
                next.remove(f.id);
              } else {
                next.add(f.id);
              }
              ref.read(selectedFileIdsProvider.notifier).state = next;
            },
            child: Row(
              children: [
                // Rounded File Badge with extension text
                Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(
                    color: Colors.white.withOpacity(0.03),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: isMedia
                      ? Image.network(
                          'http://127.0.0.1:7432/api/files/${f.id}/thumbnail',
                          fit: BoxFit.cover,
                          errorBuilder: (_, __, ___) => Center(
                            child: Text(f.fileName.split('.').last.toUpperCase(),
                                style: const TextStyle(color: accent, fontWeight: FontWeight.bold, fontSize: 10)),
                          ),
                        )
                      : Center(
                          child: Text(f.fileName.split('.').last.toUpperCase(),
                              style: const TextStyle(color: accent, fontWeight: FontWeight.bold, fontSize: 10)),
                        ),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(f.fileName,
                          style: const TextStyle(color: textPrimary, fontWeight: FontWeight.w600, fontSize: 13.5),
                          maxLines: 1, overflow: TextOverflow.ellipsis),
                      const SizedBox(height: 2),
                      Text(
                        '${(f.fileSize / 1024 / 1024).toStringAsFixed(1)} MB · ${f.downloadCount} transfers',
                        style: const TextStyle(color: textMuted, fontSize: 11),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
        if (isMedia)
          IconButton(
            icon: const Icon(Icons.visibility_rounded, color: accent, size: 18),
            onPressed: () => _previewMedia(context, f),
            tooltip: 'Preview Media',
          ),
        IconButton(
          icon: const Icon(Icons.file_download_rounded, color: success, size: 18),
          onPressed: () => _downloadFile(context, f),
          tooltip: 'Save Local',
        ),
        if (showDelete)
          IconButton(
            icon: const Icon(Icons.delete_outline_rounded, color: danger, size: 18),
            onPressed: () => ref.read(apiServiceProvider).revokeFile(f.id),
            tooltip: 'Remove',
          ),
      ],
    ),
  );
}

// Dynamic Clipboard Card Builder with base64 image support
Widget _buildClipItem(ClipboardEntry c, BuildContext context) {
  final isImage = c.contentType.startsWith('image/') || c.content.startsWith('data:image/');
  
  Widget? bodyWidget;
  if (isImage) {
    try {
      final base64Str = c.content.split(',').last;
      final bytes = base64Decode(base64Str);
      bodyWidget = Container(
        margin: const EdgeInsets.only(bottom: 6),
        constraints: const BoxConstraints(maxHeight: 130),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: Colors.white.withOpacity(0.05)),
        ),
        clipBehavior: Clip.antiAlias,
        child: Image.memory(bytes, fit: BoxFit.contain),
      );
    } catch (e) {
      bodyWidget = Text('Corrupted image payload: $e', style: const TextStyle(color: danger, fontSize: 11));
    }
  } else {
    bodyWidget = Text(
      c.content,
      style: const TextStyle(color: textPrimary, fontFamily: 'monospace', fontSize: 12),
      maxLines: 3,
      overflow: TextOverflow.ellipsis,
    );
  }

  return InkWell(
    onTap: () {
      Clipboard.setData(ClipboardData(text: c.content));
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Copied payload to clipboard'), duration: Duration(milliseconds: 900)),
      );
    },
    borderRadius: BorderRadius.circular(10),
    child: Stack(
      children: [
        Container(
          padding: const EdgeInsets.fromLTRB(16, 12, 12, 12),
          decoration: BoxDecoration(
            color: Colors.white.withOpacity(0.015),
            borderRadius: BorderRadius.circular(10),
            border: Border.all(color: Colors.white.withOpacity(0.035)),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              bodyWidget,
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                    decoration: BoxDecoration(
                      color: c.source == 'desktop' ? accent.withOpacity(0.12) : success.withOpacity(0.1),
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text(
                      c.source.toUpperCase(),
                      style: TextStyle(
                        color: c.source == 'desktop' ? accent : success,
                        fontSize: 8.5, fontWeight: FontWeight.bold,
                      ),
                    ),
                  ),
                  const Text('click to copy', style: TextStyle(color: textMuted, fontSize: 10.5, fontWeight: FontWeight.w500)),
                ],
              ),
            ],
          ),
        ),
        // Indicator tag
        Positioned(
          left: 0, top: 0, bottom: 0,
          child: Container(
            width: 3.5,
            decoration: BoxDecoration(
              gradient: isImage ? gradCyan : gradPurple,
              borderRadius: const BorderRadius.only(
                topLeft: Radius.circular(10),
                bottomLeft: Radius.circular(10),
              ),
            ),
          ),
        ),
      ],
    ),
  );
}

// Media Previews
void _previewMedia(BuildContext context, SharedFile f) {
  showDialog(
    context: context,
    builder: (context) => Dialog(
      backgroundColor: Colors.transparent,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: bgBase,
              border: Border.all(color: Colors.white.withOpacity(0.08)),
              borderRadius: BorderRadius.circular(16),
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(8),
              child: Image.network(
                'http://127.0.0.1:7432/api/files/${f.id}',
                fit: BoxFit.contain,
              ),
            ),
          ),
          Positioned(
            top: 20, right: 20,
            child: CircleAvatar(
              backgroundColor: Colors.black.withOpacity(0.5),
              child: IconButton(
                icon: const Icon(Icons.close_rounded, color: Colors.white),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

Future<void> _downloadFile(BuildContext context, SharedFile f) async {
  try {
    final savePath = await FilePicker.platform.saveFile(
      dialogTitle: 'Save File Local',
      fileName: f.fileName,
    );
    if (savePath == null) return;

    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Downloading/Saving ${f.fileName}...'),
          duration: const Duration(seconds: 1),
        ),
      );
    }

    final dio = Dio();
    await dio.download(
      'http://127.0.0.1:7432/api/files/${f.id}',
      savePath,
    );

    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Saved to: $savePath'),
          backgroundColor: success,
        ),
      );
    }
  } catch (e) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Failed to download file: $e'),
          backgroundColor: danger,
        ),
      );
    }
  }
}

// ── Clipboard Search & Paste Popup (System-wide Overlay) ─────────────────────

class ClipboardPopup extends ConsumerStatefulWidget {
  const ClipboardPopup({super.key});

  @override
  ConsumerState<ClipboardPopup> createState() => _ClipboardPopupState();
}

class _ClipboardPopupState extends ConsumerState<ClipboardPopup>
    with SingleTickerProviderStateMixin {
  final TextEditingController _searchController = TextEditingController();
  final FocusNode _searchFocusNode = FocusNode();
  int _selectedIndex = 0;
  String _searchQuery = '';

  late AnimationController _animController;
  late Animation<double> _scaleAnimation;
  late Animation<double> _fadeAnimation;

  @override
  void initState() {
    super.initState();
    _animController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 280),
    );
    _scaleAnimation = Tween<double>(begin: 0.92, end: 1.0).animate(
      CurvedAnimation(parent: _animController, curve: Curves.easeOutCubic),
    );
    _fadeAnimation = CurvedAnimation(
      parent: _animController,
      curve: Curves.easeOut,
    );
    _animController.forward();

    _searchController.addListener(() {
      if (!mounted) return;
      setState(() {
        _searchQuery = _searchController.text.toLowerCase();
        _selectedIndex = 0; // Reset selected item when query changes
      });
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _searchFocusNode.requestFocus();
      }
    });
  }

  @override
  void dispose() {
    _animController.dispose();
    _searchController.dispose();
    _searchFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final clipboard = ref.watch(clipboardHistoryProvider);
    final filtered = clipboard.where((item) {
      if (item.contentType.startsWith('image/')) {
        final ocr = (item.ocrText ?? '').toLowerCase();
        return ocr.contains(_searchQuery) || 'image'.contains(_searchQuery);
      }
      return item.content.toLowerCase().contains(_searchQuery);
    }).toList();

    _searchFocusNode.onKeyEvent = (node, event) {
      if (event is KeyDownEvent) {
        debugPrint("[onKeyEvent] Key pressed: ${event.logicalKey.debugName}");
        if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
          if (filtered.isNotEmpty) {
            setState(() {
              _selectedIndex = (_selectedIndex + 1) % filtered.length;
            });
            return KeyEventResult.handled;
          }
        } else if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
          if (filtered.isNotEmpty) {
            setState(() {
              _selectedIndex = (_selectedIndex - 1 + filtered.length) % filtered.length;
            });
            return KeyEventResult.handled;
          }
        } else if (event.logicalKey == LogicalKeyboardKey.enter ||
                   event.logicalKey == LogicalKeyboardKey.numpadEnter) {
          if (filtered.isNotEmpty && _selectedIndex < filtered.length) {
            _selectItem(filtered[_selectedIndex]);
            return KeyEventResult.handled;
          }
        } else if (event.logicalKey == LogicalKeyboardKey.escape) {
          windowManager.hide();
          return KeyEventResult.handled;
        }
      }
      return KeyEventResult.ignored;
    };

    return ScaleTransition(
      scale: _scaleAnimation,
      child: FadeTransition(
        opacity: _fadeAnimation,
        child: Scaffold(
          backgroundColor: Colors.transparent,
          body: ClipRRect(
              borderRadius: BorderRadius.circular(16),
              child: BackdropFilter(
                filter: ImageFilter.blur(sigmaX: 30, sigmaY: 30),
                child: Container(
                  decoration: BoxDecoration(
                    gradient: LinearGradient(
                      colors: [
                        const Color(0xEB131422), // Premium frosted violet-black
                        const Color(0xF2090A0E),
                      ],
                      begin: Alignment.topLeft,
                      end: Alignment.bottomRight,
                    ),
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(
                      color: const Color(0xFF818CF8).withOpacity(0.12),
                      width: 1.5,
                    ),
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withOpacity(0.65),
                        blurRadius: 28,
                        spreadRadius: 2,
                      ),
                    ],
                  ),
                  child: Column(
                    children: [
                      // Sleek Drag Area at Top
                      DragToMoveArea(
                        child: Container(
                          height: 24,
                          alignment: Alignment.center,
                          child: Container(
                            width: 32,
                            height: 4,
                            decoration: BoxDecoration(
                              color: Colors.white.withOpacity(0.12),
                              borderRadius: BorderRadius.circular(2),
                            ),
                          ),
                        ),
                      ),
                      // Search Input Field
                      Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 16),
                        child: Container(
                          decoration: BoxDecoration(
                            boxShadow: [
                              BoxShadow(
                                color: const Color(0xFF818CF8).withOpacity(0.04),
                                blurRadius: 12,
                                spreadRadius: 1,
                              ),
                            ],
                          ),
                          child: TextField(
                            controller: _searchController,
                            focusNode: _searchFocusNode,
                            style: const TextStyle(color: Colors.white, fontSize: 14, fontWeight: FontWeight.w400),
                            decoration: InputDecoration(
                              prefixIcon: Icon(
                                Icons.search_rounded,
                                color: const Color(0xFF818CF8).withOpacity(0.6),
                                size: 18,
                              ),
                              hintText: 'Search clipboard history...',
                              hintStyle: TextStyle(
                                color: Colors.white.withOpacity(0.25),
                                fontSize: 13,
                              ),
                              filled: true,
                              fillColor: Colors.white.withOpacity(0.02),
                              contentPadding: const EdgeInsets.symmetric(vertical: 10, horizontal: 14),
                              enabledBorder: OutlineInputBorder(
                                borderRadius: BorderRadius.circular(12),
                                borderSide: BorderSide(
                                  color: Colors.white.withOpacity(0.05),
                                ),
                              ),
                              focusedBorder: OutlineInputBorder(
                                borderRadius: BorderRadius.circular(12),
                                borderSide: const BorderSide(
                                  color: Color(0xFF818CF8),
                                  width: 1.5,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 12),
                      // Filtered History List
                      Expanded(
                        child: filtered.isEmpty
                            ? Center(
                                child: Text(
                                  'No matching history items',
                                  style: TextStyle(
                                    color: Colors.white.withOpacity(0.35),
                                    fontSize: 12,
                                    letterSpacing: -0.2,
                                  ),
                                ),
                              )
                            : ListView.builder(
                                itemCount: filtered.length,
                                padding: const EdgeInsets.symmetric(horizontal: 16),
                                itemBuilder: (context, index) {
                                  final item = filtered[index];
                                  final isSelected = index == _selectedIndex;
                                  return GestureDetector(
                                    behavior: HitTestBehavior.opaque,
                                    onTapDown: (_) => _selectItem(item),
                                    child: AnimatedScale(
                                      scale: isSelected ? 1.012 : 1.0,
                                      duration: const Duration(milliseconds: 140),
                                      curve: Curves.easeOutCubic,
                                      child: AnimatedContainer(
                                        duration: const Duration(milliseconds: 140),
                                        margin: const EdgeInsets.only(bottom: 8),
                                        padding: const EdgeInsets.all(12),
                                        decoration: BoxDecoration(
                                          color: isSelected
                                              ? const Color(0xFF818CF8).withOpacity(0.12)
                                              : Colors.white.withOpacity(0.015),
                                          borderRadius: BorderRadius.circular(12),
                                          border: Border.all(
                                            color: isSelected
                                                ? const Color(0xFF818CF8).withOpacity(0.4)
                                                : Colors.white.withOpacity(0.03),
                                            width: 1,
                                          ),
                                          boxShadow: isSelected
                                              ? [
                                                  BoxShadow(
                                                    color: const Color(0xFF818CF8).withOpacity(0.06),
                                                    blurRadius: 12,
                                                    spreadRadius: 1,
                                                  )
                                                ]
                                              : [],
                                        ),
                                        child: Row(
                                          children: [
                                            // Glowing focus pill on the left
                                            AnimatedContainer(
                                              duration: const Duration(milliseconds: 140),
                                              width: 3,
                                              height: isSelected ? 20 : 0,
                                              decoration: BoxDecoration(
                                                color: const Color(0xFF818CF8),
                                                borderRadius: BorderRadius.circular(1.5),
                                                boxShadow: [
                                                  BoxShadow(
                                                    color: const Color(0xFF818CF8).withOpacity(0.8),
                                                    blurRadius: 6,
                                                  ),
                                                ],
                                              ),
                                            ),
                                            SizedBox(width: isSelected ? 10 : 0),
                                            Icon(
                                              item.contentType.startsWith('image/')
                                                  ? Icons.image_outlined
                                                  : Icons.terminal_outlined,
                                              color: isSelected
                                                  ? const Color(0xFF818CF8)
                                                  : Colors.white.withOpacity(0.35),
                                              size: 16,
                                            ),
                                            const SizedBox(width: 12),
                                            Expanded(
                                              child: item.contentType.startsWith('image/')
                                                  ? _buildImagePreview(item.content)
                                                  : Text(
                                                      item.content.trim(),
                                                      style: TextStyle(
                                                        color: isSelected ? Colors.white : Colors.white.withOpacity(0.75),
                                                        fontFamily: 'monospace',
                                                        fontSize: 12,
                                                        height: 1.4,
                                                      ),
                                                      maxLines: 2,
                                                      overflow: TextOverflow.ellipsis,
                                                    ),
                                            ),
                                            if (isSelected)
                                              const Icon(
                                                Icons.keyboard_return_rounded,
                                                color: Color(0xFF818CF8),
                                                size: 14,
                                              ),
                                          ],
                                        ),
                                      ),
                                    ),
                                  );
                                },
                              ),
                      ),
                      // Bottom bar
                      Container(
                        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 16),
                        decoration: BoxDecoration(
                          color: Colors.black.withOpacity(0.25),
                          border: Border(
                            top: BorderSide(
                              color: Colors.white.withOpacity(0.04),
                            ),
                          ),
                          borderRadius: const BorderRadius.only(
                            bottomLeft: Radius.circular(16),
                            bottomRight: Radius.circular(16),
                          ),
                        ),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Text(
                              '↑↓ to navigate • ↵ to paste • ⎋ to cancel',
                              style: TextStyle(
                                color: Colors.white.withOpacity(0.35),
                                fontSize: 10,
                                letterSpacing: -0.1,
                              ),
                            ),
                            Container(
                              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: const Color(0xFF818CF8).withOpacity(0.12),
                                borderRadius: BorderRadius.circular(4),
                              ),
                              child: const Text(
                                '⌥B',
                                style: TextStyle(
                                  color: Color(0xFF818CF8),
                                  fontSize: 9,
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
      );
  }

  Widget _buildImagePreview(String base64Content) {
    try {
      final base64Str = base64Content.split(',').last;
      final bytes = base64Decode(base64Str);
      return Align(
        alignment: Alignment.centerLeft,
        child: Container(
          height: 38,
          clipBehavior: Clip.antiAlias,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: Colors.white.withOpacity(0.06)),
          ),
          child: Image.memory(bytes, fit: BoxFit.contain),
        ),
      );
    } catch (e) {
      return const Text('[Corrupted Image]', style: TextStyle(color: Colors.red, fontSize: 11));
    }
  }

  Future<void> _selectItem(ClipboardEntry item) async {
    debugPrint("[_selectItem] Selected item: ${item.content}");
    try {
      await Clipboard.setData(ClipboardData(text: item.content));
      debugPrint("[_selectItem] Set clipboard content successfully.");
      await windowManager.hide();
      debugPrint("[_selectItem] Window hidden.");
      await Future.delayed(const Duration(milliseconds: 50));
      debugPrint("[_selectItem] Delay complete. Simulating native paste...");
      if (Platform.isMacOS) {
        const channel = MethodChannel('lynqo/window_style');
        await channel.invokeMethod('simulatePaste');
        debugPrint("[_selectItem] Native paste method channel completed.");
      }
    } catch (e) {
      debugPrint("[_selectItem] Error selecting item: $e");
    }
  }
}
