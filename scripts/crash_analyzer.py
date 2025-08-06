#!/usr/bin/env python3
"""
Crash Dump Analyzer for Local Desktop Android App
Decodes and analyzes Crashpad minidumps from Android crash logs.
"""

import re
import base64
import binascii
import sys
from datetime import datetime


def extract_crashpad_data(crash_log):
    """Extract the encoded crashpad data from the crash log."""
    # Look for the crashpad minidump sections
    crashpad_pattern = r'F crashpad: ([^$\n]+)'
    matches = re.findall(crashpad_pattern, crash_log, re.MULTILINE)
    
    if not matches:
        print("❌ No Crashpad minidump data found in the crash log")
        return None
    
    # Join all the crashpad data lines
    crashpad_data = ''.join(matches)
    
    # Remove the BEGIN/END markers if present
    crashpad_data = crashpad_data.replace('-----BEGIN CRASHPAD MINIDUMP-----', '')
    crashpad_data = crashpad_data.replace('-----END CRASHPAD MINIDUMP-----', '')
    
    return crashpad_data.strip()


def decode_crashpad_data(encoded_data):
    """Attempt to decode the crashpad data using various methods."""
    if not encoded_data:
        return None
    
    print(f"📊 Encoded data length: {len(encoded_data)} characters")
    
    # Try different decoding methods
    methods = [
        ("Base64", decode_base64),
        ("Hex", decode_hex),
        ("Raw analysis", analyze_raw_data)
    ]
    
    for method_name, decode_func in methods:
        print(f"\n🔍 Trying {method_name} decoding...")
        try:
            result = decode_func(encoded_data)
            if result:
                return result
        except Exception as e:
            print(f"❌ {method_name} decoding failed: {e}")
    
    return None


def decode_base64(data):
    """Try to decode as base64."""
    try:
        # Clean the data - remove non-base64 characters
        clean_data = re.sub(r'[^A-Za-z0-9+/=]', '', data)
        decoded = base64.b64decode(clean_data)
        if decoded:
            print(f"✅ Base64 decoded {len(decoded)} bytes")
            return analyze_binary_data(decoded)
    except Exception as e:
        print(f"Base64 decoding error: {e}")
    return None


def decode_hex(data):
    """Try to decode as hexadecimal."""
    try:
        # Remove non-hex characters
        clean_data = re.sub(r'[^0-9A-Fa-f]', '', data)
        if len(clean_data) % 2 == 0:
            decoded = binascii.unhexlify(clean_data)
            print(f"✅ Hex decoded {len(decoded)} bytes")
            return analyze_binary_data(decoded)
    except Exception as e:
        print(f"Hex decoding error: {e}")
    return None


def analyze_raw_data(data):
    """Analyze the raw encoded data for patterns."""
    print("📋 Raw data analysis:")
    print(f"   • Length: {len(data)} characters")
    print(f"   • First 100 chars: {data[:100]}...")
    print(f"   • Last 100 chars: ...{data[-100:]}")
    
    # Look for common patterns
    patterns = {
        'Printable ASCII': len(re.findall(r'[!-~]', data)),
        'Digits': len(re.findall(r'\d', data)),
        'Letters': len(re.findall(r'[A-Za-z]', data)),
        'Special chars': len(re.findall(r'[^A-Za-z0-9\s]', data)),
    }
    
    print("📊 Character distribution:")
    for pattern, count in patterns.items():
        percentage = (count / len(data)) * 100
        print(f"   • {pattern}: {count} ({percentage:.1f}%)")
    
    # Look for repeating patterns
    common_substrings = find_common_substrings(data)
    if common_substrings:
        print("🔄 Common patterns found:")
        for pattern, count in common_substrings[:5]:
            print(f"   • '{pattern}' appears {count} times")
    
    return {"type": "raw_analysis", "data": data, "patterns": patterns}


def find_common_substrings(data, min_length=3, max_length=10):
    """Find common repeating substrings."""
    substring_counts = {}
    
    for length in range(min_length, min(max_length + 1, len(data))):
        for i in range(len(data) - length + 1):
            substring = data[i:i + length]
            if substring.isalnum():  # Only consider alphanumeric substrings
                substring_counts[substring] = substring_counts.get(substring, 0) + 1
    
    # Return substrings that appear more than once, sorted by frequency
    return sorted([(s, c) for s, c in substring_counts.items() if c > 1], 
                  key=lambda x: x[1], reverse=True)


def analyze_binary_data(binary_data):
    """Analyze decoded binary data."""
    print(f"🔍 Binary data analysis ({len(binary_data)} bytes):")
    
    # Check for common file signatures
    signatures = {
        b'MDMP': 'Windows Minidump',
        b'ELF': 'ELF executable',
        b'\x7fELF': 'ELF executable',
        b'PK': 'ZIP/APK archive',
        b'\x89PNG': 'PNG image',
        b'GIF8': 'GIF image',
        b'\xff\xd8\xff': 'JPEG image',
    }
    
    for sig, desc in signatures.items():
        if binary_data.startswith(sig):
            print(f"✅ Detected file type: {desc}")
            break
    else:
        print("❓ Unknown binary format")
    
    # Show hex dump of first 256 bytes
    print("\n📋 Hex dump (first 256 bytes):")
    hex_dump = binascii.hexlify(binary_data[:256]).decode('ascii')
    for i in range(0, len(hex_dump), 32):
        line = hex_dump[i:i+32]
        formatted_line = ' '.join(line[j:j+2] for j in range(0, len(line), 2))
        print(f"   {i//2:04x}: {formatted_line}")
    
    # Look for printable strings
    printable_strings = extract_strings(binary_data)
    if printable_strings:
        print(f"\n📝 Found {len(printable_strings)} printable strings:")
        for string in printable_strings[:10]:  # Show first 10
            print(f"   • {string}")
        if len(printable_strings) > 10:
            print(f"   ... and {len(printable_strings) - 10} more")
    
    return {
        "type": "binary",
        "size": len(binary_data),
        "data": binary_data,
        "strings": printable_strings
    }


def extract_strings(binary_data, min_length=4):
    """Extract printable strings from binary data."""
    strings = []
    current_string = ""
    
    for byte in binary_data:
        if 32 <= byte <= 126:  # Printable ASCII
            current_string += chr(byte)
        else:
            if len(current_string) >= min_length:
                strings.append(current_string)
            current_string = ""
    
    # Don't forget the last string
    if len(current_string) >= min_length:
        strings.append(current_string)
    
    return strings


def analyze_crash_context(crash_log):
    """Extract additional context from the crash log."""
    print("\n🕐 Crash Context Analysis:")
    
    # Extract timestamp
    timestamp_match = re.search(r'(\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3})', crash_log)
    if timestamp_match:
        timestamp = timestamp_match.group(1)
        print(f"   • Crash time: {timestamp}")
    
    # Extract process ID
    pid_match = re.search(r'(\d+)\s+\d+\s+F crashpad:', crash_log)
    if pid_match:
        pid = pid_match.group(1)
        print(f"   • Process ID: {pid}")
    
    # Look for other log entries around the crash
    lines = crash_log.split('\n')
    crashpad_line_idx = None
    
    for i, line in enumerate(lines):
        if 'F crashpad:' in line:
            crashpad_line_idx = i
            break
    
    if crashpad_line_idx:
        print("\n📋 Log context (lines before crash):")
        start_idx = max(0, crashpad_line_idx - 5)
        for i in range(start_idx, crashpad_line_idx):
            if i < len(lines) and lines[i].strip():
                print(f"   {lines[i]}")


def provide_crash_analysis_suggestions():
    """Provide suggestions for crash analysis and debugging."""
    print("\n💡 Crash Analysis Suggestions:")
    print("   1. Check if this is a known issue in the Local Desktop project")
    print("   2. Look at recent changes in the codebase that might have caused this")
    print("   3. Check Sentry dashboard for similar crashes")
    print("   4. Verify if the crash is reproducible")
    print("   5. Check device-specific information (Android version, architecture)")
    print("   6. Review memory usage and potential memory leaks")
    print("   7. Check for issues with the Wayland compositor or Xwayland")
    print("   8. Verify proot filesystem integrity")
    
    print("\n🔧 Debugging Steps:")
    print("   1. Enable debug builds with more verbose logging")
    print("   2. Use Android Studio's native debugging tools")
    print("   3. Check for stack overflow or memory corruption")
    print("   4. Review JNI interactions between Rust and Android")
    print("   5. Test on different Android devices/versions")


def main():
    """Main crash analysis function."""
    if len(sys.argv) < 2:
        print("Usage: python3 crash_analyzer.py <crash_log_text>")
        print("Or pipe the crash log: cat crash.log | python3 crash_analyzer.py")
        return
    
    # Get crash log from command line argument or stdin
    if len(sys.argv) == 2 and sys.argv[1] != "-":
        crash_log = sys.argv[1]
    else:
        crash_log = sys.stdin.read()
    
    print("🔍 Local Desktop Crash Analyzer")
    print("=" * 50)
    
    # Analyze crash context
    analyze_crash_context(crash_log)
    
    # Extract and decode crashpad data
    print("\n📦 Extracting Crashpad Data...")
    crashpad_data = extract_crashpad_data(crash_log)
    
    if crashpad_data:
        print(f"✅ Found crashpad data: {len(crashpad_data)} characters")
        
        # Decode the data
        decoded_result = decode_crashpad_data(crashpad_data)
        
        if decoded_result:
            print("\n✅ Successfully analyzed crash dump data")
        else:
            print("\n❌ Could not decode crash dump data")
    else:
        print("❌ No crashpad data found in the log")
    
    # Provide suggestions
    provide_crash_analysis_suggestions()


if __name__ == "__main__":
    main()