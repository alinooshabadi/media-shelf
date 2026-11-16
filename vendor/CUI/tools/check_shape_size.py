"""Check MSVC x64 Release hello against the fixed upstream OFF + 45 KiB ceiling."""
import argparse
from pathlib import Path

if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("off", type=Path)
    p.add_argument("on", type=Path)
    args = p.parse_args()
    off, on = args.off.stat().st_size, args.on.stat().st_size
    print(f"OFF: {off} bytes; ON: {on} bytes; ON minus OFF: {on - off} bytes")
    print("Upstream OFF: 282112; allowance: 46080; ON ceiling: 328192 bytes")
    passed = on <= 282112 + 46080
    print("PASS" if passed else "FAIL: optional text path exceeds its size budget")
    raise SystemExit(0 if passed else 1)
