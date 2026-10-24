"""Check every decoded portrait tab-switch frame for a black content region.

Use with GpuTabTransitionProbe's five-tab recordings. This smoke check does
not replace visual inspection for stale content, layout jumps or partial flashes.
"""
import argparse
import json
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("video")
parser.add_argument("--ffmpeg", default="ffmpeg")
args = parser.parse_args()
result = subprocess.run(
    [args.ffmpeg, "-v", "error", "-i", args.video, "-vf",
     "scale=108:216,crop=88:160:10:28", "-fps_mode", "passthrough",
     "-pix_fmt", "gray", "-f", "rawvideo", "-"],
    capture_output=True, check=True,
)
size = 88 * 160
if not result.stdout or len(result.stdout) % size:
    raise SystemExit("No complete video frames decoded")
black = [index for index in range(len(result.stdout) // size)
         if sum(pixel < 8 for pixel in result.stdout[index * size:(index + 1) * size])
         > size * .95]
print(json.dumps({"frames": len(result.stdout) // size, "black_content_frames": black}))
raise SystemExit(bool(black))
