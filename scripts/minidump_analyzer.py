#!/usr/bin/env python3
"""
Advanced Minidump Analyzer for Local Desktop
Analyzes the decoded binary minidump data to extract crash information.
"""

import struct
import sys
import binascii
from typing import Optional, Dict, List, Tuple


class MinidumpAnalyzer:
    """Analyzes minidump binary data to extract crash information."""
    
    def __init__(self, binary_data: bytes):
        self.data = binary_data
        self.size = len(binary_data)
        self.pos = 0
        
    def read_uint32(self) -> Optional[int]:
        """Read a 32-bit unsigned integer."""
        if self.pos + 4 > self.size:
            return None
        value = struct.unpack('<I', self.data[self.pos:self.pos + 4])[0]
        self.pos += 4
        return value
    
    def read_uint64(self) -> Optional[int]:
        """Read a 64-bit unsigned integer."""
        if self.pos + 8 > self.size:
            return None
        value = struct.unpack('<Q', self.data[self.pos:self.pos + 8])[0]
        self.pos += 8
        return value
    
    def read_bytes(self, count: int) -> Optional[bytes]:
        """Read a specific number of bytes."""
        if self.pos + count > self.size:
            return None
        value = self.data[self.pos:self.pos + count]
        self.pos += count
        return value
    
    def seek(self, position: int):
        """Seek to a specific position."""
        self.pos = position
    
    def analyze_header(self) -> Dict:
        """Analyze the minidump header."""
        print("🔍 Analyzing minidump header...")
        
        self.seek(0)
        
        # Check for MDMP signature
        signature = self.read_bytes(4)
        if signature == b'MDMP':
            print("✅ Valid MDMP minidump signature found")
            return self._parse_mdmp_header()
        else:
            print(f"❓ Unknown signature: {signature}")
            return self._analyze_unknown_format()
    
    def _parse_mdmp_header(self) -> Dict:
        """Parse a standard MDMP minidump header."""
        header = {}
        
        # Version
        version = self.read_uint32()
        if version:
            header['version'] = version
            print(f"   • Version: 0x{version:08x}")
        
        # Number of streams
        stream_count = self.read_uint32()
        if stream_count:
            header['stream_count'] = stream_count
            print(f"   • Stream count: {stream_count}")
        
        # RVA to stream directory
        stream_directory_rva = self.read_uint32()
        if stream_directory_rva:
            header['stream_directory_rva'] = stream_directory_rva
            print(f"   • Stream directory RVA: 0x{stream_directory_rva:08x}")
        
        return header
    
    def _analyze_unknown_format(self) -> Dict:
        """Analyze data that doesn't match standard minidump format."""
        print("📋 Analyzing unknown binary format...")
        
        analysis = {
            'format': 'unknown',
            'size': self.size,
            'entropy': self._calculate_entropy(),
            'patterns': self._find_patterns()
        }
        
        print(f"   • Data size: {self.size} bytes")
        print(f"   • Entropy: {analysis['entropy']:.3f}")
        
        if analysis['patterns']:
            print("   • Repeating patterns found:")
            for pattern, count in analysis['patterns'][:5]:
                print(f"     - {pattern.hex()}: {count} occurrences")
        
        return analysis
    
    def _calculate_entropy(self) -> float:
        """Calculate the entropy of the data."""
        import math
        if not self.data:
            return 0.0
        
        # Count byte frequencies
        freq = [0] * 256
        for byte in self.data:
            freq[byte] += 1
        
        # Calculate entropy
        entropy = 0.0
        for count in freq:
            if count > 0:
                p = count / len(self.data)
                entropy -= p * math.log2(p)
        
        return entropy
    
    def _find_patterns(self) -> List[Tuple[bytes, int]]:
        """Find repeating byte patterns."""
        patterns = {}
        
        # Look for 2-4 byte patterns
        for pattern_len in range(2, 5):
            for i in range(len(self.data) - pattern_len + 1):
                pattern = self.data[i:i + pattern_len]
                patterns[pattern] = patterns.get(pattern, 0) + 1
        
        # Return patterns that appear more than once
        return sorted([(p, c) for p, c in patterns.items() if c > 1], 
                     key=lambda x: x[1], reverse=True)
    
    def extract_strings(self, min_length: int = 4) -> List[str]:
        """Extract printable strings from the data."""
        strings = []
        current = ""
        
        for byte in self.data:
            if 32 <= byte <= 126:  # Printable ASCII
                current += chr(byte)
            else:
                if len(current) >= min_length:
                    strings.append(current)
                current = ""
        
        if len(current) >= min_length:
            strings.append(current)
        
        return strings
    
    def analyze_potential_addresses(self) -> List[int]:
        """Look for potential memory addresses in the data."""
        addresses = []
        
        # Look for 4-byte and 8-byte aligned values that could be addresses
        for i in range(0, len(self.data) - 8, 4):
            # Try 32-bit address
            addr32 = struct.unpack('<I', self.data[i:i+4])[0]
            if 0x10000000 <= addr32 <= 0xFFFFFFFF:  # Reasonable address range
                addresses.append(addr32)
            
            # Try 64-bit address
            if i + 8 <= len(self.data):
                addr64 = struct.unpack('<Q', self.data[i:i+8])[0]
                if 0x100000000 <= addr64 <= 0x7FFFFFFFFFFF:  # 64-bit address range
                    addresses.append(addr64)
        
        return list(set(addresses))  # Remove duplicates
    
    def search_for_crash_info(self) -> Dict:
        """Search for crash-related information in the data."""
        print("\n🔍 Searching for crash information...")
        
        crash_info = {
            'strings': self.extract_strings(),
            'potential_addresses': self.analyze_potential_addresses(),
            'suspicious_patterns': []
        }
        
        # Look for common crash-related strings
        crash_keywords = [
            b'SIGSEGV', b'SIGABRT', b'SIGBUS', b'SIGFPE', b'SIGILL',
            b'segfault', b'abort', b'crash', b'exception', b'fault',
            b'stack', b'heap', b'memory', b'null', b'access',
            b'libandroid', b'libc.so', b'libm.so', b'liblog.so'
        ]
        
        for keyword in crash_keywords:
            if keyword in self.data:
                pos = self.data.find(keyword)
                crash_info['suspicious_patterns'].append({
                    'keyword': keyword.decode('ascii', errors='ignore'),
                    'position': pos,
                    'context': self.data[max(0, pos-10):pos+len(keyword)+10]
                })
        
        # Print findings
        if crash_info['strings']:
            print(f"   • Found {len(crash_info['strings'])} strings:")
            for s in crash_info['strings'][:10]:
                print(f"     - '{s}'")
        
        if crash_info['potential_addresses']:
            print(f"   • Found {len(crash_info['potential_addresses'])} potential addresses:")
            for addr in crash_info['potential_addresses'][:10]:
                print(f"     - 0x{addr:x}")
        
        if crash_info['suspicious_patterns']:
            print(f"   • Found {len(crash_info['suspicious_patterns'])} crash-related patterns:")
            for pattern in crash_info['suspicious_patterns']:
                print(f"     - '{pattern['keyword']}' at position {pattern['position']}")
        
        return crash_info


def analyze_crash_from_hex_dump():
    """Analyze the crash from the hex dump we extracted earlier."""
    # This is the hex data from the crash analyzer output
    hex_data = """8b 22 67 8a 2b 27 c6 71 cd ec 9d 4d 1b f5 4a e1
f9 c6 c4 d3 10 93 fc b8 02 5f b2 7d e5 e4 9c cf
38 3b df 41 0c 57 5e 8c 0b d6 75 fc 6c 73 8b f8
e6 b6 fa 7c 45 1d a1 40 d2 83 a8 36 e6 a9 a5 d1
bd d8 82 87 8d 97 fd c2 07 4c 7a 26 ff af b5 17
04 cf ff 2b 0e 73 30 76 a2 af 9c 4d 51 3f 00 38
0c 92 17 35 17 1d 3f af 29 5e c5 be c4 6c 90 08
78 2a 9f 95 da 76 f7 d5 d0 33 19 f8 78 37 66 65
ad b2 e6 01 01 3d 97 20 a4 b2 27 e2 10 f3 cd 44
b1 ec 5b b7 9f 3c f4 b8 1d 29 ed 15 f8 60 d6 31
ac 37 fd ff 6b f1 a3 4f 3d 84 e2 6d cc e7 2b 50
0b b1 a9 5b e0 e1 2b 06 79 78 2b 22 80 7f 40 e6
ea 98 88 8c e3 fa 7d a0 2b af 0d ed 3e 5c 34 b9
37 dd e4 cc 5c fd 6b 01 56 9d 48 05 e2 d4 7e 9a
a0 b7 8c 1f ae 30 7f 4c d9 6c a9 ef ba ec 07 fa
93 52 9e 55 30 67 e5 a2 a2 61 31 95 30 7a f9 45"""
    
    # Convert hex string to bytes
    hex_clean = hex_data.replace('\n', ' ').replace('  ', ' ')
    binary_data = bytes.fromhex(hex_clean)
    
    print("🔍 Advanced Minidump Analysis")
    print("=" * 50)
    
    analyzer = MinidumpAnalyzer(binary_data)
    
    # Analyze header
    header_info = analyzer.analyze_header()
    
    # Search for crash information
    crash_info = analyzer.search_for_crash_info()
    
    # Additional analysis
    print("\n📊 Additional Analysis:")
    print(f"   • Data appears to be: {'compressed/encrypted' if analyzer._calculate_entropy() > 7.0 else 'uncompressed'}")
    
    return {
        'header': header_info,
        'crash_info': crash_info,
        'analyzer': analyzer
    }


def provide_local_desktop_specific_analysis():
    """Provide analysis specific to Local Desktop crashes."""
    print("\n🏠 Local Desktop Specific Analysis:")
    print("   This crash occurred in the Local Desktop Android app, which:")
    print("   • Runs a Wayland compositor in Android NDK")
    print("   • Uses Proot to mount an Arch Linux filesystem")
    print("   • Launches Xwayland and desktop environment in chroot")
    print("   • Uses JNI to interact between Rust and Android")
    
    print("\n🎯 Common Local Desktop Crash Causes:")
    print("   1. Memory issues in the Wayland compositor")
    print("   2. Proot filesystem corruption or mounting issues")
    print("   3. Xwayland crashes or display server problems")
    print("   4. JNI boundary issues between Rust and Java")
    print("   5. OpenGL/EGL context issues with Android graphics")
    print("   6. Permission issues accessing Android resources")
    print("   7. Architecture-specific ARM64 compatibility issues")
    
    print("\n🔧 Recommended Investigation Steps:")
    print("   1. Check logcat for additional Android system logs")
    print("   2. Verify the Arch Linux filesystem integrity")
    print("   3. Test with different desktop environments (not just XFCE)")
    print("   4. Check for memory pressure on the Android device")
    print("   5. Verify OpenGL ES support and drivers")
    print("   6. Test on different Android versions/devices")
    print("   7. Enable native debugging symbols in the build")


if __name__ == "__main__":
    result = analyze_crash_from_hex_dump()
    provide_local_desktop_specific_analysis()
    
    print("\n✅ Analysis Complete")
    print("   Review the findings above and check the Local Desktop")
    print("   project's issue tracker for similar crashes.")