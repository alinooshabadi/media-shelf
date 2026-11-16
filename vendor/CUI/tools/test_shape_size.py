"""The current OFF binary must never raise the fixed upstream ON ceiling."""
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        off, on = root / "off.exe", root / "on.exe"
        off.write_bytes(b"0" * 400000)
        script = Path(__file__).with_name("check_shape_size.py")
        for size, status in ((328192, 0), (328193, 1)):
            on.write_bytes(b"0" * size)
            result = subprocess.run([sys.executable, str(script), str(off), str(on)],
                                    capture_output=True, text=True)
            assert result.returncode == status, result.stdout + result.stderr
    print("fixed upstream ceiling: boundary and inflated-OFF checks passed")


if __name__ == "__main__":
    main()
