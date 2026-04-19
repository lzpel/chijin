"""M2ネジを CadQuery で生成し STEP + STL + PNG出力する。PNGを見てネジになっているかを確認する。らせん構造の再現が必要"""

import math
from pathlib import Path

import cadquery as cq
import pyvista as pv

# ISO M2 概略仕様 (mm)
D = 2.0          # 呼び径 (外径)
PITCH = 0.4      # ピッチ
THREAD_LEN = 6.0 # ねじ部長さ
HEAD_D = 3.5     # 頭部直径
HEAD_H = 1.3     # 頭部厚さ

# ISO メートルねじ山高さ H = (sqrt(3)/2) * p, 実効高さ ~ 5/8 H
H_TRI = (math.sqrt(3) / 2.0) * PITCH


def build_m2_screw() -> cq.Workplane:
    radius_major = D / 2.0
    # 軸部 (谷径より少し小さい円柱を中心に、ねじ山を盛る)
    root_radius = radius_major - (5.0 / 8.0) * H_TRI
    shaft = cq.Workplane("XY").circle(root_radius).extrude(THREAD_LEN)

    # ヘリックス (path) を作成
    helix_wire = cq.Wire.makeHelix(
        pitch=PITCH,
        height=THREAD_LEN,
        radius=root_radius,
    )
    helix_path = cq.Workplane(obj=helix_wire)

    # ねじ山断面: 半径方向 X、高さ方向 Z の三角形
    # 始点はヘリックス始点 (root_radius, 0, 0) に一致させる
    thread_height = (5.0 / 8.0) * H_TRI
    half_p = PITCH / 2.0
    profile = (
        cq.Workplane("XZ")
        .moveTo(root_radius, -half_p)
        .lineTo(root_radius + thread_height, 0)
        .lineTo(root_radius, half_p)
        .close()
    )

    thread = profile.sweep(helix_path, isFrenet=True)

    threaded_shaft = shaft.union(thread)

    # 上下端を平らに整える (ねじ部の長さで box intersect)
    bbox_clip = (
        cq.Workplane("XY")
        .box(D * 2, D * 2, THREAD_LEN, centered=(True, True, False))
    )
    threaded_shaft = threaded_shaft.intersect(bbox_clip)

    # 頭部 (なべ頭簡略: 円柱)
    head = (
        cq.Workplane("XY")
        .workplane(offset=THREAD_LEN)
        .circle(HEAD_D / 2.0)
        .extrude(HEAD_H)
    )

    screw = threaded_shaft.union(head)
    return screw


def export_step_stl(model: cq.Workplane, stem: Path) -> None:
    cq.exporters.export(model, str(stem.with_suffix(".step")))
    cq.exporters.export(
        model,
        str(stem.with_suffix(".stl")),
        tolerance=0.01,
        angularTolerance=0.1,
    )


def render_png(stl_path: Path, png_path: Path) -> None:
    mesh = pv.read(str(stl_path))
    pl = pv.Plotter(off_screen=True, window_size=(800, 800))
    pl.add_mesh(mesh, color="lightgray", smooth_shading=True, show_edges=False)
    pl.camera_position = "iso"
    pl.enable_shadows()
    pl.screenshot(str(png_path))


if __name__ == "__main__":
    out_dir = Path(__file__).parent
    stem = out_dir / "m2_screw"

    model = build_m2_screw()
    export_step_stl(model, stem)
    render_png(stem.with_suffix(".stl"), stem.with_suffix(".png"))
    print(f"wrote: {stem}.step / .stl / .png")
