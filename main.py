# Keep running in a terminal. The "generate checkerboard" program on screenshot will trigger this script to try detecting
# the checkerboard in the image.

import os
import re
import time
from typing import override

import cv2
from watchdog.events import DirCreatedEvent, FileCreatedEvent, FileSystemEventHandler
from watchdog.observers import Observer

INPUT_DIR = "screenshots"
OUTPUT_DIR = "screenshot-detections"
SUPPORTED_EXTS = (".png", ".jpg", ".jpeg", ".bmp", ".tiff")

PATH_RE = re.compile(r"checkerboard_(\d+)x(\d+)", re.IGNORECASE)


processed_files: set[str] = set()  # to avoid double-processing


def cols_rows_from_filename(filename: str):
    """Return (cols, rows) (inner corners) from filename if present, else None."""
    m = PATH_RE.search(filename)
    if not m:
        return None
    try:
        # Subtract one since the stored info is cols/rows not inner corners
        cols = int(m.group(1)) - 1
        rows = int(m.group(2)) - 1
        return (cols, rows)
    except ValueError:
        return None


def is_image_file(path: str):
    return path.lower().endswith(SUPPORTED_EXTS)


def already_processed(path: str):
    """Decide whether to skip processing this input file.
    Currently: skip if there's already an output file with same basename in OUTPUT_DIR.
    """
    basename = os.path.basename(path)
    out_path = os.path.join(OUTPUT_DIR, basename)
    return os.path.exists(out_path)


def process_image(path: str):
    """Look for checkerboard corners, draw them and save result to OUTPUT_DIR."""
    try:
        if not is_image_file(path):
            return

        # If output exists, assume already processed
        if already_processed(path):
            print(f"[SKIP] Output already exists for {path}")
            return

        print(f"[INFO] Processing: {path}")

        # Wait briefly to reduce chance of reading partial writes
        time.sleep(0.2)

        image = cv2.imread(path)
        if image is None:
            print(f"[WARN] Could not read image (maybe still writing?): {path}")
            return

        gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)

        # per-file override?
        basename = os.path.basename(path)
        cols_rows = cols_rows_from_filename(basename)
        if cols_rows is None:
            print(f"[WARN] Could not read cols, rows from filename: {path}")
            return

        # OpenCV expects size as (columns, rows) inner corners
        flags_sb = cv2.CALIB_CB_NORMALIZE_IMAGE | cv2.CALIB_CB_EXHAUSTIVE

        corners = None
        print(f"[INFO] Looking for inner corners (cols, rows)={cols_rows}..")
        found, corners = cv2.findChessboardCornersSB(gray, cols_rows, corners, flags_sb)

        if found:
            # Improve corner accuracy
            criteria = (cv2.TERM_CRITERIA_EPS + cv2.TERM_CRITERIA_MAX_ITER, 30, 0.001)
            corners_refined = cv2.cornerSubPix(
                gray, corners, (11, 11), (-1, -1), criteria
            )

            # Draw found corners onto the image
            drawn = cv2.drawChessboardCorners(image, cols_rows, corners_refined, found)

            out_path = os.path.join(OUTPUT_DIR, basename)
            success = cv2.imwrite(out_path, drawn)
            if success:
                print(f"[OK] Checkerboard found and saved to: {out_path}")
                processed_files.add(path)
            else:
                print(f"[ERR] Failed to write output for {path}")
        else:
            print(f"[MISS] No checkerboard found in {path}")

    except Exception as e:
        print(f"[ERROR] Exception processing {path}: {e}")


class ImageHandler(FileSystemEventHandler):
    """React to new image files in the watched directory."""

    @override
    def on_created(self, event: DirCreatedEvent | FileCreatedEvent):
        # filesystem events can be noisy; ignore directories
        if event.is_directory:
            return
        src = str(event.src_path)
        if not is_image_file(src):
            print(f"path wasn't image file: {src}")
            return

        # Small wait to allow writer to finish
        time.sleep(0.5)

        # Avoid re-processing same path twice during runtime
        if src in processed_files or already_processed(src):
            return
        process_image(src)


def process_existing_inputs():
    """Scan INPUT_DIR at startup and process any unprocessed image files."""
    for fname in sorted(os.listdir(INPUT_DIR)):
        path = os.path.join(INPUT_DIR, fname)
        if not os.path.isfile(path):
            continue
        if not is_image_file(path):
            continue
        # skip if already processed (output exists)
        if already_processed(path):
            print(f"[SKIP] Already processed (output exists): {path}")
            continue
        process_image(path)


def main():
    print(f"Watching folder: {os.path.abspath(INPUT_DIR)}")
    # First process existing files that haven't been processed
    process_existing_inputs()

    # Then start watcher for new arrivals
    event_handler = ImageHandler()
    observer = Observer()
    _ = observer.schedule(event_handler, INPUT_DIR, recursive=False)
    observer.start()

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("Stopping...")
        observer.stop()
    observer.join()


if __name__ == "__main__":
    main()
