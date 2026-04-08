"""M2ネジ（軸のみ）を CadQuery で生成し STEP + SVG 出力する。"""

import cadquery as cq
import math

# ---- M2 パラメータ (ISO 724 / JIS B 0205) ----
d = 2.0          # 呼び径 (mm)
pitch = 0.4      # ピッチ (mm)
length = 6.0     # ネジ部長さ (mm)

# ネジ山パラメータ
H = pitch * math.sqrt(3) / 2          # 理論山高さ
d_minor = d - 2 * (5 / 8) * H         # 谷径
thread_depth = (d - d_minor) / 2       # 実際の山深さ

# ---- 1. ネジ軸 (谷径の円柱) ----
shaft = (
    cq.Workplane("XY")
    .circle(d_minor / 2)
    .extrude(length)
)

# ---- 2. ネジ山: 螺旋パス + 三角断面 sweep ----
helix = cq.Wire.makeHelix(
    pitch=pitch,
    height=length,
    radius=d / 2,
)

# ネジ山の三角断面
tri_pts = [
    (0, 0),
    (-pitch / 2, -thread_depth),
    (pitch / 2, -thread_depth),
]
thread_profile = (
    cq.Workplane("XZ")
    .workplane(offset=d / 2)
    .polyline(tri_pts)
    .close()
    .wire()
)

thread_solid = cq.Solid.sweep(
    thread_profile.val(),
    [],
    helix,
    True,   # makeSolid
    True,   # isFrenet
)

# ---- 3. 合成 ----
bolt = shaft.union(cq.Workplane("XY").add(thread_solid))

# ---- 4. 出力 ----
import os
out = os.path.join(os.path.dirname(__file__), "out")
os.makedirs(out, exist_ok=True)

cq.exporters.export(bolt, os.path.join(out, "m2_thread.step"))
cq.exporters.export(bolt, os.path.join(out, "m2_thread.stl"))
cq.exporters.export(bolt, os.path.join(out, "m2_thread.svg"), opt={
    "width": 400,
    "height": 400,
    "projectionDir": (1, 1, 0.5),
    "showHidden": False,
    "showAxes": False,
    "strokeWidth": 0.5,
})
print(f"Exported to {out}: m2_thread.step, m2_thread.stl, m2_thread.svg")
