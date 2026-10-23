import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';

class QrContactScreen extends StatelessWidget {
  final String contactUri;
  final Function(String scannedUri) onContactScanned;

  const QrContactScreen({
    Key? key,
    required this.contactUri,
    required this.onContactScanned,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 2,
      child: Scaffold(
        backgroundColor: const Color(0xFF101418),
        appBar: AppBar(
          backgroundColor: const Color(0xFF1A2028),
          title: const Text('Contact Link & QR Exchange'),
          bottom: const TabBar(
            tabs: [
              Tab(icon: Icon(Icons.qr_code_2_rounded), text: 'My QR Code'),
              Tab(icon: Icon(Icons.qr_code_scanner_rounded), text: 'Scan Partner'),
            ],
          ),
        ),
        body: TabBarView(
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
                      ),
                      child: QrImageView(
                        data: contactUri,
                        version: QrVersions.auto,
                        size: 260.0,
                      ),
                    ),
                    const SizedBox(height: 24),
                    const Text(
                      'Scan this code to establish a metadata-free, post-quantum pairwise queue over Tor.',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: Color(0xFF8E99A4), fontSize: 13),
                    ),
                  ],
                ),
              ),
            ),
            // Tab 2: Manual Link / Scanner Stub
            Center(
              child: Padding(
                padding: const EdgeInsets.all(24.0),
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Icon(Icons.camera_alt_outlined, size: 64, color: Colors.white38),
                    const SizedBox(height: 16),
                    const Text(
                      'Import Contact Link',
                      style: TextStyle(color: Colors.white, fontSize: 18, fontWeight: FontWeight.bold),
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      style: const TextStyle(color: Colors.white, fontSize: 13),
                      decoration: InputDecoration(
                        hintText: 'Paste simplex:/contact#/?... URI',
                        hintStyle: const TextStyle(color: Colors.white24),
                        filled: true,
                        fillColor: const Color(0xFF1E242C),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(12),
                          borderSide: BorderSide.none,
                        ),
                      ),
                      onSubmitted: (val) {
                        if (val.trim().isNotEmpty) {
                          onContactScanned(val.trim());
                          Navigator.pop(context);
                        }
                      },
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
