# Keep running in a terminal. The "generate checkerboard" program on screenshot will trigger this script to try detecting
# the checkerboard in the image.

from math import asin, atan2, degrees
from cv2.typing import MatLike
import numpy as np
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

PATH_RE = re.compile(
    r"checkerboard_rows=(\d+)_cols=(\d+)_squares_mm=(\d+)", re.IGNORECASE
)


processed_files: set[str] = set()  # to avoid double-processing

global_object_points: dict[tuple[int, int], list[MatLike]] = {}
global_image_points: dict[tuple[int, int], list[MatLike]] = {}


def append_corners(
    cols_rows: tuple[int, int], refined_corners: MatLike, squares_mm: float
):
    cols, rows = cols_rows

    objp = np.zeros((cols * rows, 3), np.float32)
    # objp[:, :2] = np.mgrid[0:rows, 0:cols].T.reshape(-1, 2) * (squares_mm * 1e-3)
    objp[:, :2] = np.mgrid[0:cols, 0:rows].T.reshape(-1, 2) * (squares_mm * 1e-3)

    global_object_points.setdefault(cols_rows, []).append(objp)
    global_image_points.setdefault(cols_rows, []).append(refined_corners)


def cols_rows_squares_mm_from_filename(filename: str):
    """Return (cols, rows) (inner corners) from filename if present, else None."""
    m = PATH_RE.search(filename)
    if not m:
        return None
    try:
        # Subtract one since the stored info is cols/rows not inner corners
        rows = int(m.group(1)) - 1
        cols = int(m.group(2)) - 1
        mm = float(m.group(3))
        return ((cols, rows), mm)
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


def rvec_to_euler_xyz(rvec):
    """Convert Rodrigues rotation vector to extrinsic XYZ Euler angles (in degrees)."""
    R, _ = cv2.Rodrigues(rvec)
    # Extrinsic XYZ means rotations about world X, Y, Z in that order.
    # Assuming a right-handed coordinate system.

    sy = -R[2, 0]
    cy = np.sqrt(1 - sy**2)

    if cy > 1e-6:
        x = atan2(R[2, 1], R[2, 2])
        y = asin(sy)
        z = atan2(R[1, 0], R[0, 0])
    else:
        # Gimbal lock
        x = atan2(-R[1, 2], R[1, 1])
        y = asin(sy)
        z = 0

    return np.array([degrees(a) for a in (x, y, z)])


def process_image(path: str):
    """Look for checkerboard corners, draw them and save result to OUTPUT_DIR."""
    try:
        if not is_image_file(path):
            return

        # If output exists, assume already processed
        if already_processed(path):
            print(f"[SKIP] Output already exists for {path}")
            return

        basename = os.path.basename(path)
        ret = cols_rows_squares_mm_from_filename(basename)
        if ret is None:
            print(f"[WARN] Could not read cols, rows, mm from filename: {path}")
            return
        cols_rows, squares_mm = ret

        print(
            f"--------------------------------------------------------------------------------"
        )
        # print(f"--- Image #{len(global_image_points[cols_rows]) + 1} ---")
        print(
            f"--------------------------------------------------------------------------------"
        )

        print(f"[INFO] Processing: {path}")

        # Wait briefly to reduce chance of reading partial writes
        time.sleep(0.2)

        image = cv2.imread(path)
        if image is None:
            print(f"[WARN] Could not read image (maybe still writing?): {path}")
            return

        gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)

        # per-file override?

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

            append_corners(cols_rows, corners_refined, squares_mm)

            # Draw found corners onto the image
            drawn = cv2.drawChessboardCorners(image, cols_rows, corners_refined, found)

            out_path = os.path.join(OUTPUT_DIR, basename)
            success = cv2.imwrite(out_path, drawn)

            if success:
                print(f"[OK] Checkerboard found and saved to: {out_path}")
                processed_files.add(path)
            else:
                print(f"[ERR] Failed to write output for {path}")
                return

            print(f"[INFO] Calibrating for size {cols_rows}")
            ret, mtx, dist, rvecs, tvecs = cv2.calibrateCamera(
                global_object_points[cols_rows],
                global_image_points[cols_rows],
                gray.shape[::-1],
                None,
                None,
            )
            # print(
            #     f"[OK] ret={ret}\nmtx={mtx}\ndist={dist}\nrvecs={rvecs}\ntvecs={tvecs}"
            # )
            print(f"[OK] ret={ret}\nmtx={mtx}\ndist={dist}")

            def make_transform(rvec, tvec):
                R, _ = cv2.Rodrigues(rvec)
                T = np.eye(4, dtype=np.float64)
                T[:3, :3] = R
                T[:3, 3] = tvec.flatten()
                return T

            T0 = make_transform(rvecs[0], tvecs[0])
            T0_inv = np.linalg.inv(T0)

            rvecs_last = rvecs[-1]
            tvecs_last = tvecs[-1]
            print(f"rvecs last: {rvecs_last}")
            print(f"tvecs last: {tvecs_last}")

            # for idx, (rvec, tvec) in enumerate(zip(rvecs, tvecs)):
            T = make_transform(rvecs_last, tvecs_last)
            T_rel = T0_inv @ T  # aka "return to T0, then apply T"
            r_rel, _ = cv2.Rodrigues(T_rel[:3, :3])
            t_rel = T_rel[:3, 3]

            euler = rvec_to_euler_xyz(r_rel)
            x, y, z = t_rel.ravel()
            print(f"Position relative to first [mm]: X={x:.2f}, Y={y:.2f}, Z={z:.2f}")
            print(
                f"Rotation relative to first [deg]: X={euler[0]:.2f}, Y={euler[1]:.2f}, Z={euler[2]:.2f}"
            )
            print("\n\n")

            objpoints = global_object_points[cols_rows][-1]
            imgpoints = global_image_points[cols_rows][-1]
            for i, (objp_i, imgp_i) in enumerate(zip(objpoints, imgpoints)):
                imgpoints_reproj, _ = cv2.projectPoints(
                    objp_i, rvecs_last, tvecs_last, mtx, dist
                )

                # Draw reprojected points
                for x, y in imgpoints_reproj.reshape(-1, 2):
                    cv2.circle(drawn, (int(x), int(y)), 3, (0, 255, 255), -1)

                # Draw lines between measured and reprojected points
                for m, r in zip(imgp_i.reshape(-1, 2), imgpoints_reproj.reshape(-1, 2)):
                    cv2.line(
                        drawn, tuple(np.int32(m)), tuple(np.int32(r)), (255, 0, 0), 1
                    )

                # Draw origin
                origin_3d = np.array([[0.0, 0.0, 0.0]], dtype=np.float32)
                origin_2d, _ = cv2.projectPoints(
                    origin_3d, rvecs_last, tvecs_last, mtx, dist
                )
                ox, oy = origin_2d.ravel().astype(int)
                cv2.circle(drawn, (ox, oy), 10, (0, 0, 255), -1)
                cv2.putText(
                    drawn,
                    "Origin",
                    (ox + 10, oy - 10),
                    cv2.FONT_HERSHEY_SIMPLEX,
                    0.5,
                    (0, 0, 255),
                    1,
                )

                # Optional reprojection error label
                # err = cv2.norm(imgp_i, imgpoints_reproj, cv2.NORM_L2) / len(
                #     imgpoints_reproj
                # )
                # cv2.putText(
                #     drawn,
                #     f"Reproj err: {err:.3f}px",
                #     (20, 30),
                #     cv2.FONT_HERSHEY_SIMPLEX,
                #     0.7,
                #     (255, 255, 255),
                #     2,
                # )

            # cv2.imwrite(f"debug_reproj.png", drawn)

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
