import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:qr_flutter/qr_flutter.dart';

class QrContactScreen extends StatefulWidget {
  final String contactUri;
  final Function(String scannedUri) onContactScanned;

  const QrContactScreen({
    Key? key,
    required this.contactUri,
    required this.onContactScanned,
  }) : super(key: key);

  @override
  State<QrContactScreen> createState() => _QrContactScreenState();
}

class _QrContactScreenState extends State<QrContactScreen>
    with SingleTickerProviderStateMixin, WidgetsBindingObserver {
  final TextEditingController _linkController = TextEditingController();
  late final TabController _tabController;
  late final MobileScannerController _scannerController;
  bool _useCamera = true;
  bool _isCopied = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _tabController = TabController(length: 2, vsync: this);
    _scannerController = MobileScannerController(
      autoStart: false,
      detectionSpeed: DetectionSpeed.noDuplicates,
      facing: CameraFacing.back,
      returnImage: false,
      formats: const [BarcodeFormat.qrCode],
    );

    _tabController.addListener(() {
      if (_tabController.index == 1 && _useCamera) {
        _scannerController.start();
      } else {
        _scannerController.stop();
      }
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed && _tabController.index == 1 && _useCamera) {
      _scannerController.start();
    } else if (state == AppLifecycleState.paused || state == AppLifecycleState.inactive) {
      _scannerController.stop();
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _tabController.dispose();
    _scannerController.dispose();
    _linkController.dispose();
    super.dispose();
  }

  void _submitLink(String link) {
    final trimmed = link.trim();
    if (trimmed.isNotEmpty) {
      _scannerController.stop();
      widget.onContactScanned(trimmed);
      Navigator.pop(context);
    }
  }

  Future<void> _pasteFromClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (data?.text != null && data!.text!.trim().isNotEmpty) {
      _linkController.text = data.text!.trim();
      _submitLink(data.text!.trim());
    }
  }

  void _copyMyLink() {
    Clipboard.setData(ClipboardData(text: widget.contactUri));
    setState(() => _isCopied = true);
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Contact Link copied to clipboard!'),
        duration: Duration(seconds: 2),
      ),
    );
    Future.delayed(const Duration(seconds: 2), () {
      if (mounted) setState(() => _isCopied = false);
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF101418),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A2028),
        title: const Text('Contact Link & QR Exchange'),
        bottom: TabBar(
          controller: _tabController,
          indicatorColor: const Color(0xFF2C6BED),
          tabs: const [
            Tab(icon: Icon(Icons.qr_code_2_rounded), text: 'My QR Code'),
            Tab(icon: Icon(Icons.qr_code_scanner_rounded), text: 'Scan Partner'),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabController,
        children: [
          // Tab 1: Show My QR Code
          Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(24.0),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Container(
                    padding: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      color: Colors.white,
                      borderRadius: BorderRadius.circular(20),
                      boxShadow: [
                        BoxShadow(
                          color: Colors.black.withOpacity(0.3),
                          blurRadius: 10,
                          offset: const Offset(0, 4),
                        ),
                      ],
                    ),
                    child: QrImageView(
                      data: widget.contactUri,
                      version: QrVersions.auto,
                      size: 240.0,
                    ),
                  ),
                  const SizedBox(height: 20),
                  ElevatedButton.icon(
                    style: ElevatedButton.styleFrom(
                      backgroundColor: const Color(0xFF2C6BED),
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
                    ),
                    icon: Icon(_isCopied ? Icons.check : Icons.copy_rounded, size: 18),
                    label: Text(_isCopied ? 'Link Copied!' : 'Copy Contact Link'),
                    onPressed: _copyMyLink,
                  ),
                  const SizedBox(height: 16),
                  const Text(
                    'Scan this code to establish a metadata-free, post-quantum pairwise queue over Tor.',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: Color(0xFF8E99A4), fontSize: 13),
                  ),
                ],
              ),
            ),
          ),
          // Tab 2: Scan Partner via Camera or Paste Link
          Column(
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    ChoiceChip(
                      label: const Text('Camera Scanner'),
                      selected: _useCamera,
                      onSelected: (val) {
                        setState(() {
                          _useCamera = true;
                          _scannerController.start();
                        });
                      },
                      selectedColor: const Color(0xFF2C6BED),
                      labelStyle: TextStyle(color: _useCamera ? Colors.white : Colors.white60),
                    ),
                    const SizedBox(width: 12),
                    ChoiceChip(
                      label: const Text('Paste Link'),
                      selected: !_useCamera,
                      onSelected: (val) {
                        setState(() {
                          _useCamera = false;
                          _scannerController.stop();
                        });
                      },
                      selectedColor: const Color(0xFF2C6BED),
                      labelStyle: TextStyle(color: !_useCamera ? Colors.white : Colors.white60),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: _useCamera
                    ? Stack(
                        alignment: Alignment.center,
                        children: [
                          MobileScanner(
                            controller: _scannerController,
                            fit: BoxFit.cover,
                            placeholderBuilder: (context, child) => const Center(
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  CircularProgressIndicator(color: Color(0xFF2C6BED)),
                                  SizedBox(height: 16),
                                  Text(
                                    'Initializing camera sensor...',
                                    style: TextStyle(color: Colors.white70, fontSize: 13),
                                  ),
                                ],
                              ),
                            ),
                            errorBuilder: (context, error, child) {
                              return Center(
                                child: Padding(
                                  padding: const EdgeInsets.all(24.0),
                                  child: Column(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      const Icon(Icons.videocam_off_rounded,
                                          size: 64, color: Colors.amberAccent),
                                      const SizedBox(height: 16),
                                      const Text(
                                        'Camera Access Blocked or Unavailable',
                                        style: TextStyle(
                                          color: Colors.white,
                                          fontSize: 16,
                                          fontWeight: FontWeight.bold,
                                        ),
                                        textAlign: TextAlign.center,
                                      ),
                                      const SizedBox(height: 8),
                                      Text(
                                        error.errorDetails?.message ??
                                            'Please grant Camera permission in your phone App Settings to scan partner QR codes directly.',
                                        textAlign: TextAlign.center,
                                        style: const TextStyle(color: Colors.white70, fontSize: 13),
                                      ),
                                      const SizedBox(height: 24),
                                      ElevatedButton.icon(
                                        style: ElevatedButton.styleFrom(
                                          backgroundColor: const Color(0xFF2C6BED),
                                          foregroundColor: Colors.white,
                                          padding: const EdgeInsets.symmetric(
                                              horizontal: 20, vertical: 12),
                                        ),
                                        icon: const Icon(Icons.refresh_rounded),
                                        label: const Text('Retry Camera'),
                                        onPressed: () => _scannerController.start(),
                                      ),
                                      const SizedBox(height: 12),
                                      TextButton.icon(
                                        style: TextButton.styleFrom(
                                            foregroundColor: Colors.tealAccent),
                                        icon: const Icon(Icons.paste_rounded),
                                        label: const Text('Or Switch to Paste Link'),
                                        onPressed: () {
                                          setState(() {
                                            _useCamera = false;
                                            _scannerController.stop();
                                          });
                                        },
                                      ),
                                    ],
                                  ),
                                ),
                              );
                            },
                            onDetect: (capture) {
                              final barcodes = capture.barcodes;
                              for (final barcode in barcodes) {
                                if (barcode.rawValue != null &&
                                    barcode.rawValue!.trim().isNotEmpty) {
                                  _submitLink(barcode.rawValue!);
                                  break;
                                }
                              }
                            },
                          ),
                          Container(
                            width: 250,
                            height: 250,
                            decoration: BoxDecoration(
                              border: Border.all(color: const Color(0xFF2C6BED), width: 3),
                              borderRadius: BorderRadius.circular(16),
                            ),
                          ),
                          Positioned(
                            bottom: 24,
                            child: Container(
                              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                              decoration: BoxDecoration(
                                color: Colors.black87,
                                borderRadius: BorderRadius.circular(20),
                              ),
                              child: const Text(
                                'Point camera at partner QR code',
                                style: TextStyle(color: Colors.white, fontSize: 13),
                              ),
                            ),
                          ),
                        ],
                      )
                    : Center(
                        child: SingleChildScrollView(
                          padding: const EdgeInsets.all(24.0),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              const Icon(Icons.link_rounded, size: 64, color: Color(0xFF2C6BED)),
                              const SizedBox(height: 16),
                              const Text(
                                'Import Partner Contact Link',
                                style: TextStyle(
                                    color: Colors.white, fontSize: 18, fontWeight: FontWeight.bold),
                              ),
                              const SizedBox(height: 16),
                              TextField(
                                controller: _linkController,
                                style: const TextStyle(color: Colors.white, fontSize: 13),
                                decoration: InputDecoration(
                                  hintText: 'simplex:/contact#/?v=1&relay=...&queue=...',
                                  hintStyle: const TextStyle(color: Colors.white24),
                                  filled: true,
                                  fillColor: const Color(0xFF1E242C),
                                  border: OutlineInputBorder(
                                    borderRadius: BorderRadius.circular(12),
                                    borderSide: BorderSide.none,
                                  ),
                                  suffixIcon: IconButton(
                                    icon: const Icon(Icons.paste_rounded, color: Color(0xFF2C6BED)),
                                    tooltip: 'Paste from Clipboard',
                                    onPressed: _pasteFromClipboard,
                                  ),
                                ),
                                onSubmitted: _submitLink,
                              ),
                              const SizedBox(height: 20),
                              SizedBox(
                                width: double.infinity,
                                height: 48,
                                child: ElevatedButton(
                                  style: ElevatedButton.styleFrom(
                                    backgroundColor: const Color(0xFF2C6BED),
                                    foregroundColor: Colors.white,
                                    shape: RoundedRectangleBorder(
                                      borderRadius: BorderRadius.circular(12),
                                    ),
                                  ),
                                  onPressed: () => _submitLink(_linkController.text),
                                  child: const Text('Connect Partner',
                                      style: TextStyle(fontWeight: FontWeight.bold)),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
