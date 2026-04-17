//! Backend-independent trait definitions.
//!
//! Trait hierarchy:
//!
//! ```text
//! Transform  ─┬─  Compound   ──  SolidStruct  (pub(crate))
//!             └─  Wire    ──  EdgeStruct   (pub(crate))
//! ```
//!
//! `Face` 型はトレイトを持たない opaque な query handle で、`tshape_id` のみ
//! を inherent method として公開する。`SolidStruct::type Face` の bound にも
//! 何も付かない。
//!
//! `Edge` / `Vec<Edge>` の対称関係は `Solid` / `Vec<Solid>` と同じ:
//!   - 単一エッジ向け constructor は `EdgeStruct` (cube/sphere に対応)
//!   - エッジ列 (= Wire) を含む共通操作は `Wire` (volume/clean に対応)
//!   - `Vec<Edge>` がそのまま Wire — 専用型は無い (`Vec<Solid>` = Compound と同様)
//!
//! - `Transform` (crate-internal): spatial ops (translate/rotate/scale/mirror).
//!   Geometry-agnostic. Implemented for shapes (`Solid`, `Edge`) and collections
//!   (`Vec<T>`, `[T; N]`). Not re-exported from `lib.rs`, so external users cannot
//!   name it — they reach the same methods via `Compound` / `Wire` forwarders.
//!
//! - `Compound: Transform` (pub): solid-specific operations on Solid, Vec<T>, and [T; N]
//!   (clean/volume/contains/color/boolean wrappers). Also exposes 1-line forwarders
//!   for every Transform method so `use cadrum::Compound;` alone enables
//!   `vec.translate(...)` etc. on `Vec<Solid>` / `[Solid; N]`.
//!
//! - `SolidStruct: Sized + Clone + Compound` (pub(crate)): backend implementation trait.
//!   Adds Solid-only operations (constructors, topology accessors, boolean primitives).
//!   build_delegation.rs parses this and generates pub inherent methods on Solid,
//!   walking the supertrait chain so all `Compound` and `Transform` methods are also
//!   exposed inherently. Trait name follows `<Type>Struct` convention (SolidStruct → Solid).
//!
//! ## 関連型による型ヒエラルキー（バックエンド非依存ルール）
//!
//! このファイルはバックエンド（occt / pure）の具象型を一切名指ししない。具象型への
//! 参照はすべて **関連型** 経由にすること。`use crate::{Edge, Face, Solid};` を
//! このファイルに書いてはいけない（書くと、両バックエンドが同時に存在する将来構成で
//! どちらの型を指すか曖昧になる）。
//!
//! ### 階層と関連型の向き
//!
//! 型同士の依存に **一方向の階層** を導入し、上位が下位を関連型として参照する：
//!
//! ```text
//!   EdgeStruct       ← 単独。下位を一切知らない
//!   SolidStruct      ← type Edge: EdgeStruct;  type Face;  (Face は bound 無し)
//!   IoModule         ← type Solid: SolidStruct;
//! ```
//!
//! 下位（Edge/Face）→ 上位（Solid）への参照は持たせない。例えば「Edge を sweep して
//! Solid を作る」操作は `EdgeStruct::sweep` ではなく `SolidStruct::sweep(profile, spine)`
//! として上位側に置き、ヒエラルキーを保つ。逆向き参照を導入する瞬間に associated type
//! の循環や Backend バンドルトレイトが必要になり、build_delegation.rs のテキスト処理が
//! 追従できなくなる。
//!
//! ### 命名と build_delegation の対応
//!
//! - `SolidStruct` の `type Edge` / `type Face`、`IoModule` の `type Solid` という名前は
//!   build_delegation.rs の `TYPE_MAP` と一致させること。`Self::Edge` / `Self::Face` /
//!   `Self::Solid` は生成時にバックエンドの具象型名（`Edge` / `Face` / `Solid`）へ
//!   置換され、`lib.rs` の `pub use occt::{Solid, Edge, Face};` により実体に解決される。
//! - 戻り型・引数型は `Vec<Self::Edge>`、`impl IntoIterator<Item = &'a Self::Solid>` の
//!   ように常に関連型経由で書く。
//! - associated type 宣言（`type Foo: Bound;`）はパーサーが行頭でスキップするので、
//!   メソッドと同じインデントで 1 行に収めること。
//!
//! パーサー挙動と制約（build_delegation.rs — 行ベースのテキスト処理）:
//!
//! トレイトヘッダ:
//! - `pub trait Foo: A + B + C {` から名前と supertrait リスト（`+` 区切り）を抽出する
//! - `Foo` が `Struct`/`Module` サフィックスを持つトレイトの supertrait に出現した場合、
//!   `Foo` のメソッドも親側の inherent impl に取り込まれる（再帰的に祖先まで辿る）
//! - 解析対象トレイト一覧に存在しない名前（`Sized`, `Clone`, ライフタイム束縛 `'a` 等）は
//!   黙って無視される
//! - 同名メソッドは子トレイト優先で重複排除される（親のオーバーライド）
//! - ヘッダ行は1行に収めること（`where` 句を改行して書くと検出されない）
//!
//! メソッドシグネチャ:
//! - fn シグネチャは1行に収めること（`where` 句・ライフタイム・ジェネリクス引数も同じ行）
//! - default impl はサポート。本体が1行に収まる場合はそのまま、複数行の場合も
//!   `{...}` ブロックを brace 深さでスキップする
//! - ライフタイム引数 `<'a, 'b>` および `where Self: 'a` のような句はそのまま保持される。
//!   `Self` は inherent impl 文脈では具象型と等価なので置換せず残す（`Self::Elem` のような
//!   関連型のみ事前に concrete type へ置換される）
//! - `Self::Elem` は impl 対象の具象型へ置換される。`Self::Face` / `Self::Edge` /
//!   `Self::Solid` はそれぞれ `Face` / `Edge` / `Solid` へ置換され、`lib.rs` の
//!   バックエンド再エクスポートで解決される
//!
//! その他:
//! - `#[cfg(...)]` は直前1行のみ認識し、続く fn に付与される
//! - `type Foo;` などの associated type 宣言は無視される（メソッド生成対象外）

#[cfg(feature = "color")]
use crate::common::color::Color;
use crate::common::error::Error;
use crate::common::mesh::Mesh;
use glam::{DMat3, DQuat, DVec3};

// ==================== Transform ====================

/// Spatial-transform operations: translate / rotate / scale / mirror.
///
/// Orthogonal to any specific geometry kind. Implemented for individual
/// shapes (`Solid`, `Edge`) and for collections (`Vec<T>`, `[T; N]`) where the
/// element type is itself `Transform`.
///
/// **Visibility**: this trait is declared `pub` but the enclosing `traits`
/// module is `pub(crate)` in `lib.rs`, and `Transform` is intentionally NOT
/// re-exported at the crate root. External users therefore cannot name it and
/// cannot `use` it. They reach the same methods through `Compound` / `Wire`,
/// which declare 1-line forwarders (`fn translate(self, ...) -> Self {
/// <Self as Transform>::translate(self, ...) }`) as default methods. This
/// keeps transforms a single source of truth inside the crate while letting
/// `use cadrum::Compound;` alone expose them externally (including on
/// collections like `Vec<Solid>` where method resolution would otherwise
/// require an import).
///
/// For `Solid` / `Edge` themselves the forwarders are unnecessary —
/// `build_delegation.rs` walks the supertrait chain and emits inherent
/// methods, so no trait import is needed on the single types.
///
/// TODO(#90): the per-method forwarders in `Compound` / `Wire` are
/// mechanical and could be generated. A future refactor could extend
/// `build_delegation.rs` (or introduce a proc-macro) to auto-emit
/// `fn foo(self, ..) -> Self { <Self as Transform>::foo(self, ..) }` for
/// every method of a referenced trait, so that Transform's surface is
/// listed exactly once in this file. Not urgent — see the issue for
/// priority notes.
pub trait Transform: Sized {
	fn translate(self, translation: DVec3) -> Self;
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
	fn rotate_x(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::X, angle) }
	fn rotate_y(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Y, angle) }
	fn rotate_z(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Z, angle) }
	fn scale(self, center: DVec3, factor: f64) -> Self;
	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self;
	/// Rotate so that local +X axis aligns with `new_x`, with local +Y projected toward `y_hint`.
	/// Rotation is around the world origin. Panics on degenerate input (zero or parallel vectors).
	fn align_x(self, new_x: DVec3, y_hint: DVec3) -> Self {
		let x = new_x.try_normalize().expect("align_x: new_x is zero");
		let z = x.cross(y_hint).try_normalize().expect("align_x: y_hint parallel to new_x");
		let (axis, angle) = DQuat::from_mat3(&DMat3::from_cols(x, z.cross(x), z)).to_axis_angle();
		self.rotate(DVec3::ZERO, axis, angle)
	}
	/// Rotate so that local +Y axis aligns with `new_y`, with local +Z projected toward `z_hint`.
	/// Rotation is around the world origin. Panics on degenerate input (zero or parallel vectors).
	fn align_y(self, new_y: DVec3, z_hint: DVec3) -> Self {
		let y = new_y.try_normalize().expect("align_y: new_y is zero");
		let x = y.cross(z_hint).try_normalize().expect("align_y: z_hint parallel to new_y");
		let (axis, angle) = DQuat::from_mat3(&DMat3::from_cols(x, y, x.cross(y))).to_axis_angle();
		self.rotate(DVec3::ZERO, axis, angle)
	}
	/// Rotate so that local +Z axis aligns with `new_z`, with local +X projected toward `x_hint`.
	/// Rotation is around the world origin. Panics on degenerate input (zero or parallel vectors).
	fn align_z(self, new_z: DVec3, x_hint: DVec3) -> Self {
		let z = new_z.try_normalize().expect("align_z: new_z is zero");
		let y = z.cross(x_hint).try_normalize().expect("align_z: x_hint parallel to new_z");
		let (axis, angle) = DQuat::from_mat3(&DMat3::from_cols(y.cross(z), y, z)).to_axis_angle();
		self.rotate(DVec3::ZERO, axis, angle)
	}
}

// ==================== ProfileOrient ====================

/// Controls how the cross-section profile is oriented as it travels along the
/// spine in [`SolidStruct::sweep`] and [`SolidStruct::sweep_sections`].
///
/// **どれを選ぶか:**
///
/// | やりたいこと | 選ぶ variant |
/// |---|---|
/// | 直線押し出し / profile を回したくない | [`Fixed`](Self::Fixed) |
/// | ねじ・バネ・つる (helix 系) | [`Torsion`](Self::Torsion) または [`Up`](Self::Up)`(axis)` |
/// | 道路・線路・パイプ (重力方向を保ちたい) | [`Up`](Self::Up)`(DVec3::Z)` |
/// | 上記に当てはまらない 3D 自由曲線 | [`Torsion`](Self::Torsion) |
/// | 任意の捻り制御 (メビウスの輪等) | [`Auxiliary`](Self::Auxiliary)`(&aux_spine)` |
///
/// **`Torsion` と `Up(axis)` の関係**: helix のような定曲率・定 torsion 曲線では、
/// この 2 つは数学的に等価なトリヘドロンを生成します。`Torsion` は曲線の主法線
/// (= `d²C/dt²` の T 直交成分) に profile を貼り付け、`Up` はユーザが渡した
/// 方向を T 直交平面に射影して binormal にする — helix 上ではこの 2 つが
/// 同じ axis を指すため、結果が一致します。helix 以外の曲線では一致しません。
#[derive(Clone, Copy)]
pub enum ProfileOrient<'a> {
	/// Profile is parallel-transported along the spine **without rotating**.
	/// All cross-sections stay parallel to the starting orientation.
	///
	/// - **適**: 直線 spine (押し出し)
	/// - **不適**: 曲がる spine (profile が tangent と直交しなくなり、見た目が壊れる)
	Fixed,

	/// Profile rotates following the spine's principal normal direction
	/// (= the T-perpendicular component of `d²C/dt²`). Equivalent to OCCT's
	/// raw Frenet–Serret frame.
	///
	/// - **適**: helix, spring, screw thread, twisted ribbon — 定曲率・
	///   定 torsion な曲線、および 3D 自由曲線で「曲線の自然な捻れ」を
	///   profile に反映させたいケース
	/// - **不適**: 変曲点 (curvature → 0) を含む 2D / 3D スプライン。
	///   変曲点で N が不定になり profile が 180° flip しうる。その場合は
	///   `Up` を使う
	Torsion,

	/// Profile keeps the given direction as its "up" axis (binormal).
	///
	/// - **適**: 道路 (`up = DVec3::Z`), 線路, パイプ, 運河 — 重力方向を
	///   保ちたい sweep 全般
	/// - **不適**: 任意の点で `up` が tangent と平行になる spine
	Up(DVec3),

	/// Profile orientation is controlled by an auxiliary spine curve.
	/// The profile's X axis tracks the direction toward the auxiliary spine.
	///
	/// - **適**: メビウスの輪、ステラレーターの断面回転、任意の捻り制御
	Auxiliary(&'a [crate::Edge]),
}

// ==================== BSplineEnd ====================

/// End-condition selector for [`EdgeStruct::bspline`].
///
/// A cubic B-spline interpolating N data points has 4(N−1) coefficient
/// degrees of freedom. The interpolation conditions plus C¹/C² continuity
/// at internal knots fix all but **2** of those. This enum chooses how
/// the remaining 2 degrees are determined.
///
/// **どれを選ぶか:**
///
/// | やりたいこと | 選ぶ variant |
/// |---|---|
/// | 閉じた断面プロファイル (プラズマ poloidal section, 自由曲線リング) | [`Periodic`](Self::Periodic) |
/// | 開いた自由曲線で端点接線が分からない | [`NotAKnot`](Self::NotAKnot) |
/// | 開いた自由曲線で端点接線が物理的に決まっている | [`Clamped`](Self::Clamped) |
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BSplineEnd {
	/// Build a periodic curve. Start and end coincide with **C² continuity**
	/// (position + tangent + curvature all match at the wrap-around).
	/// The first data point must NOT be repeated at the end — periodicity
	/// is encoded in the basis function structure. Passing a duplicated
	/// endpoint yields [`Error::InvalidEdge`].
	///
	/// Requires ≥ 3 distinct points. The most common choice for closed
	/// profile curves (plasma poloidal sections, screw threads, gear teeth)
	/// where the start/end seam should be invisible.
	Periodic,

	/// Open curve, end conditions chosen so that the cubics on the first
	/// two intervals collapse into a single cubic (and likewise at the
	/// other end). The 2nd and (N−1)th data points behave as plain
	/// interpolation targets that do not act as real knots.
	///
	/// This is the default in MATLAB, SciPy, and OCCT itself. Best when
	/// nothing is known about end behavior — gives the most "natural"
	/// looking boundary because the boundary cubic is fit to 3 data
	/// points instead of being constrained by an artificial derivative
	/// condition. Requires ≥ 2 points.
	NotAKnot,

	/// Open curve with explicit start/end tangent vectors. The magnitude
	/// of each vector controls how strongly the curve is pulled along
	/// that direction near the boundary — a unit vector gives a gentle
	/// hint, a longer vector pulls more aggressively. Requires ≥ 2 points.
	Clamped {
		start: DVec3,
		end: DVec3,
	},
}

// ==================== Wire / EdgeStruct ====================

/// Public trait: edge/wire-level operations on `Edge`, `Vec<Edge>` and `[Edge; N]`.
///
/// `Vec<Edge>` plays the role of a wire in this library — there is no
/// dedicated `Wire` type, mirroring how `Compound` is just `Vec<Solid>`.
/// Methods on `Wire` therefore have meaningful semantics for both a single
/// edge and an ordered edge list:
///
/// - `start_point` / `start_tangent` — the wire's starting position/direction.
///   For a single edge, the edge's first point and tangent.
///   For a `Vec<Edge>`, the first edge's start.
/// - `is_closed` — does the geometry form a closed loop?
///   For a single edge, whether start == end (e.g. a circle).
///   For a `Vec<Edge>`, whether the first edge's start equals the last edge's end.
/// - `approximation_segments` — polyline approximation. For a wire, all
///   sub-edges' segments are concatenated in order.
///
/// Spatial transforms live on the (crate-private) supertrait `Transform`.
/// Since `Transform` is not re-exported from the crate root, users cannot
/// bring it into scope directly. Instead `Wire` exposes 1-line forwarders
/// for every `Transform` method as default methods, so `use cadrum::Wire;`
/// alone enables `vec_of_edges.translate(...)` etc.
///
/// As with `Compound`, `EdgeStruct: Wire` so users of `Edge` get these
/// methods inherently via `build_delegation.rs`; the `use` import is only
/// required when chaining on `Vec<Edge>` / `[Edge; N]`.
pub trait Wire: Transform {
	type Elem: EdgeStruct;

	fn start_point(&self) -> DVec3;
	fn start_tangent(&self) -> DVec3;
	fn is_closed(&self) -> bool;
	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3>;

	// --- Transform forwarders ---
	// Let `use cadrum::Wire;` alone pull the Transform surface into scope.
	// TODO(#90): auto-generate these from `Transform` (extend
	// build_delegation.rs or introduce a proc-macro) so the list doesn't have
	// to be mirrored by hand. See the `Transform` doc comment for the design
	// note. Not urgent.
	fn translate(self, translation: DVec3) -> Self { <Self as Transform>::translate(self, translation) }
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self { <Self as Transform>::rotate(self, axis_origin, axis_direction, angle) }
	fn rotate_x(self, angle: f64) -> Self { <Self as Transform>::rotate_x(self, angle) }
	fn rotate_y(self, angle: f64) -> Self { <Self as Transform>::rotate_y(self, angle) }
	fn rotate_z(self, angle: f64) -> Self { <Self as Transform>::rotate_z(self, angle) }
	fn scale(self, center: DVec3, factor: f64) -> Self { <Self as Transform>::scale(self, center, factor) }
	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self { <Self as Transform>::mirror(self, plane_origin, plane_normal) }
	fn align_x(self, new_x: DVec3, y_hint: DVec3) -> Self { <Self as Transform>::align_x(self, new_x, y_hint) }
	fn align_y(self, new_y: DVec3, z_hint: DVec3) -> Self { <Self as Transform>::align_y(self, new_y, z_hint) }
	fn align_z(self, new_z: DVec3, x_hint: DVec3) -> Self { <Self as Transform>::align_z(self, new_z, x_hint) }
}

/// Backend-independent edge trait (pub(crate) — not exposed to users).
///
/// Single-edge constructors only. Wire/edge-list operations live on `Wire`
/// and are inherited via the supertrait bound, in symmetry with `SolidStruct`.
///
/// All constructors return `Result<..., Error>`. Invalid inputs (degenerate
/// geometry, zero/negative radius, collinear arc points, etc.) yield
/// `Error::InvalidEdge(String)` with a message that identifies the failing
/// constructor and the offending parameters.
pub trait EdgeStruct: Sized + Clone + Wire {
	/// Construct a single helical edge on a cylindrical surface centered at
	/// the world origin.
	///
	/// - `radius`: cylinder radius
	/// - `pitch`: rise per full revolution
	/// - `height`: total rise (number of turns = `height / pitch`)
	/// - `axis`: cylinder axis direction (must be non-zero)
	/// - `x_ref`: reference direction that anchors the local +X axis of the
	///   cylindrical frame. The helix start point is
	///   `radius * normalize(component of x_ref orthogonal to axis)`.
	///   `x_ref` must not be parallel to `axis`.
	///
	/// Making `x_ref` explicit guarantees the start point is deterministic
	/// rather than depending on whatever orthogonal direction OCCT picks
	/// from `axis` alone.
	fn helix(radius: f64, pitch: f64, height: f64, axis: DVec3, x_ref: DVec3) -> Result<Self, Error>;

	/// Build a closed polygon from a sequence of points and return its
	/// constituent edges in order. The polygon is **always closed**: the
	/// last point is automatically connected back to the first.
	// 非平面の点列も受理する (検証しない) — `Solid::sweep` で face 化に失敗
	// したとき `Error::SweepFailed` で気付ける想定なので、入力側での事前検査は省略。
	fn polygon<'a>(points: impl IntoIterator<Item = &'a DVec3>) -> Result<Vec<Self>, Error>;

	/// Closed circle of radius `r` centered at the world origin, lying in
	/// the plane normal to `axis`. Returns a single edge (one Geom_Circle
	/// curve — not a polygon approximation).
	///
	/// The circle's start/end point (at which `start_point()` /
	/// `start_tangent()` are evaluated) is chosen by the backend from an
	/// arbitrary orthogonal direction to `axis`. Callers that need a
	/// deterministic start point should translate/rotate the resulting
	/// edge into place rather than relying on the implicit choice.
	fn circle(radius: f64, axis: DVec3) -> Result<Self, Error>;

	/// Straight line segment from `a` to `b`. Fails with `InvalidEdge` if
	/// `a == b` (zero-length segment).
	fn line(a: DVec3, b: DVec3) -> Result<Self, Error>;

	/// Circular arc through three points: start, mid, end. The unique circle
	/// passing through the three points defines the arc; `mid` disambiguates
	/// which of the two possible arcs is returned (the one passing through
	/// `mid`). Fails with `InvalidEdge` if `mid` is collinear with `start`
	/// and `end`, or if any pair of points coincides.
	fn arc_3pts(start: DVec3, mid: DVec3, end: DVec3) -> Result<Self, Error>;

	/// Cubic B-spline curve interpolating the given data points.
	///
	/// **The points are interpolation targets, not control points.** OCCT's
	/// `GeomAPI_Interpolate` solves a linear system so the resulting curve
	/// passes through every input point exactly. The internal control points
	/// and knots are computed automatically and not exposed.
	///
	/// - Degree: 3 (cubic)
	/// - Parameterization: chord-length
	/// - End behavior: chosen by `end` (see [`BSplineEnd`])
	///
	/// Returns one `Edge` wrapping a single `Geom_BSplineCurve`. Use as a
	/// sweep/loft section by wrapping in `vec![...]` or `&[edge]`.
	///
	/// # Errors
	///
	/// Returns [`Error::InvalidEdge`] if:
	/// - point count is below the minimum (≥3 for `Periodic`, ≥2 otherwise)
	/// - `BSplineEnd::Periodic` is requested but the first and last points
	///   coincide (periodicity is encoded in the basis; do not duplicate)
	/// - OCCT's interpolation fails (degenerate point distribution, etc.)
	fn bspline<'a>(points: impl IntoIterator<Item = &'a DVec3>, end: BSplineEnd) -> Result<Self, Error>;
}

/// Backend-independent solid trait (pub(crate) — not exposed to users).
///
/// `Solid`-specific operations only. The shared methods (transforms, queries,
/// color, boolean wrappers) live on `Compound` and are inherited via the
/// supertrait bound.
///

/// build_delegation.rs generates `impl Solid { pub fn ... }` from this trait
/// and walks the supertrait chain to expose `Compound` methods inherently as well.
///
/// Associated types `Edge`/`Face` keep this trait backend-independent: each
/// backend (occt / pure) binds them to its own concrete types in the impl.
pub trait SolidStruct: Sized + Clone + Compound {
	type Edge: EdgeStruct;
	type Face;

	// --- Constructors ---
	fn cube(x: f64, y: f64, z: f64) -> Self;
	fn sphere(radius: f64) -> Self;
	fn cylinder(r: f64, axis: DVec3, h: f64) -> Self;
	fn cone(r1: f64, r2: f64, axis: DVec3, h: f64) -> Self;
	fn torus(r1: f64, r2: f64, axis: DVec3) -> Self;
	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Self;

	// --- Topology ---
	fn faces(&self) -> Vec<Self::Face>;
	fn edges(&self) -> Vec<Self::Edge>;

	/// Extrude a closed profile wire along a direction vector to form a solid.
	///
	/// Internally builds a face from the wire and uses `BRepPrimAPI_MakePrism`.
	/// Fails if the profile is empty, not closed, or the direction is zero-length.
	fn extrude<'a>(profile: impl IntoIterator<Item = &'a Self::Edge>, dir: DVec3) -> Result<Self, Error> where Self::Edge: 'a;

	// --- Sweep ---
	/// Sweep a closed profile wire (= ordered edge list) along a spine wire
	/// to create a solid. Both inputs are accepted as `IntoIterator` of edge
	/// references so a single `&Edge` (via `std::slice::from_ref`) and a
	/// `&Vec<Edge>` work uniformly.
	///
	/// The profile must be closed; otherwise the underlying pipe operation
	/// produces a shell rather than a solid and an error is returned.
	///
	/// `orient` selects how the profile is oriented along the spine. See
	/// [`ProfileOrient`] for the trade-offs between [`Fixed`](ProfileOrient::Fixed),
	/// [`Torsion`](ProfileOrient::Torsion), and [`Up`](ProfileOrient::Up).
	// 戻り型は単一 `Self` 固定。MakePipeShell が compound を返すことは closed
	// face 入力に対しては実質起きないため、`Vec<Self>` に拡張する手間を省いた。
	// 想定外ケースに当たったら `Solid::new` の debug_assert で気付ける。
	fn sweep<'a, 'b, 'c>(profile: impl IntoIterator<Item = &'a Self::Edge>, spine: impl IntoIterator<Item = &'b Self::Edge>, orient: ProfileOrient<'c>) -> Result<Self, Error> where Self::Edge: 'a + 'b;

	/// Loft (skin) a smooth solid through a sequence of cross-section wires.
	///
	/// Each `section` is an ordered list of edges forming a closed wire (a
	/// "rib"). The lofter interpolates a B-spline surface through all sections
	/// in order, then caps the ends to form a `Solid`.
	///
	/// OCCT caps the first/last sections with planar faces to form a closed
	/// solid (the standard "trunk" / "frustum" shape).
	///
	/// Internally uses `BRepOffsetAPI_ThruSections(isSolid=true, isRuled=false)`.
	fn loft<'a, S, I>(sections: S) -> Result<Self, Error> where S: IntoIterator<Item = I>, I: IntoIterator<Item = &'a Self::Edge>, Self::Edge: 'a;

	/// Build a B-spline surface solid from a 2D control-point grid.
	///
	/// `grid[i][j]` — index `i` (0..M) runs along the longitudinal (U) direction,
	/// index `j` (0..N) runs along the cross-section (V) direction. V is always
	/// periodic (the cross-section is a closed loop); U is periodic iff
	/// `periodic=true`, producing a torus. When `periodic=false` the U-ends are
	/// capped with planar faces, producing a pipe.
	///
	/// Internally builds a `Geom_BSplineSurface` via `GeomAPI_PointsToBSplineSurface::Interpolate`
	/// on an augmented grid (first row/column duplicated at the end to satisfy
	/// the `SetUPeriodic`/`SetVPeriodic` pole-matching precondition), then
	/// wraps the surface in a face, caps it if needed, sews, and makes a solid.
	fn bspline<const M: usize, const N: usize>(grid: [[DVec3; N]; M], periodic: bool) -> Result<Self, Error>;

	// --- Boolean primitives (consumed by Compound::*_with_metadata wrappers) ---
	fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
}

// ==================== Compound ====================

/// Public trait: solid-specific operations on Solid, Vec<Solid>, and [Solid; N].
///
/// Spatial transforms (translate/rotate/scale/mirror) live on the crate-private
/// supertrait `Transform`. `Compound` re-exposes them through 1-line
/// forwarders as default methods, so `use cadrum::Compound;` alone is enough
/// to call `vec.translate(...)` / `[a,b].rotate_z(...)` on collections — no
/// separate `Transform` import is needed (and none is possible from outside
/// the crate).
pub trait Compound: Transform {
	type Elem: SolidStruct;

	fn clean(&self) -> Result<Self, Error>;

	// --- Transform forwarders ---
	// Let `use cadrum::Compound;` alone pull the Transform surface into scope.
	// TODO(#90): auto-generate these from `Transform` (extend
	// build_delegation.rs or introduce a proc-macro) so the list doesn't have
	// to be mirrored by hand. See the `Transform` doc comment for the design
	// note. Not urgent.
	fn translate(self, translation: DVec3) -> Self { <Self as Transform>::translate(self, translation) }
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self { <Self as Transform>::rotate(self, axis_origin, axis_direction, angle) }
	fn rotate_x(self, angle: f64) -> Self { <Self as Transform>::rotate_x(self, angle) }
	fn rotate_y(self, angle: f64) -> Self { <Self as Transform>::rotate_y(self, angle) }
	fn rotate_z(self, angle: f64) -> Self { <Self as Transform>::rotate_z(self, angle) }
	fn scale(self, center: DVec3, factor: f64) -> Self { <Self as Transform>::scale(self, center, factor) }
	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self { <Self as Transform>::mirror(self, plane_origin, plane_normal) }
	fn align_x(self, new_x: DVec3, y_hint: DVec3) -> Self { <Self as Transform>::align_x(self, new_x, y_hint) }
	fn align_y(self, new_y: DVec3, z_hint: DVec3) -> Self { <Self as Transform>::align_y(self, new_y, z_hint) }
	fn align_z(self, new_z: DVec3, x_hint: DVec3) -> Self { <Self as Transform>::align_z(self, new_z, x_hint) }

	// --- Queries ---
	fn volume(&self) -> f64;
	fn bounding_box(&self) -> [DVec3; 2];
	fn contains(&self, point: DVec3) -> bool;
	fn shell_count(&self) -> u32;

	// --- Color ---
	#[cfg(feature = "color")]
	fn color(self, color: impl Into<Color>) -> Self;
	#[cfg(feature = "color")]
	fn color_clear(self) -> Self;

	// --- Boolean (-> Vec<Self::Elem>) ---
	fn union_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn subtract_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn intersect_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn union<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.union_with_metadata(tool)?.0) }
	fn subtract<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.subtract_with_metadata(tool)?.0) }
	fn intersect<'a>(&self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.intersect_with_metadata(tool)?.0) }
}

// `impl Compound for Solid` lives in the backend module (e.g. src/occt/solid.rs)
// because it needs direct access to the backend FFI for the per-element operations.

// ==================== impl Transform / Compound for Vec<T> ====================

impl<T: Transform> Transform for Vec<T> {
	fn translate(self, v: DVec3) -> Self { self.into_iter().map(|s| s.translate(v)).collect() }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.into_iter().map(|s| s.rotate(o, d, a)).collect() }
	fn scale(self, c: DVec3, f: f64) -> Self { self.into_iter().map(|s| s.scale(c, f)).collect() }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.into_iter().map(|s| s.mirror(o, n)).collect() }
}

impl<T: SolidStruct> Compound for Vec<T> {
	type Elem = T;
	fn clean(&self) -> Result<Self, Error> { self.iter().map(|s| s.clean()).collect() }
	fn volume(&self) -> f64 { self.iter().map(|s| s.volume()).sum() }
	fn bounding_box(&self) -> [DVec3; 2] {
		self.iter().map(|s| s.bounding_box())
			.reduce(|[amin, amax], [bmin, bmax]| [amin.min(bmin), amax.max(bmax)])
			.unwrap_or([DVec3::ZERO, DVec3::ZERO])
	}
	fn contains(&self, p: DVec3) -> bool { self.iter().any(|s| s.contains(p)) }
	fn shell_count(&self) -> u32 { self.iter().map(|s| s.shell_count()).sum() }
	#[cfg(feature = "color")]
	fn color(self, color: impl Into<Color>) -> Self {
		let c: Color = color.into();
		self.into_iter().map(|s| s.color(c)).collect()
	}
	#[cfg(feature = "color")]
	fn color_clear(self) -> Self {
		self.into_iter().map(|s| s.color_clear()).collect()
	}
	fn union_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_union(self.iter(), tool)
	}
	fn subtract_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_subtract(self.iter(), tool)
	}
	fn intersect_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_intersect(self.iter(), tool)
	}
}

// ==================== impl Transform / Compound for [T; N] ====================

impl<T: Transform, const N: usize> Transform for [T; N] {
	fn translate(self, v: DVec3) -> Self { self.map(|s| s.translate(v)) }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.map(|s| s.rotate(o, d, a)) }
	fn scale(self, c: DVec3, f: f64) -> Self { self.map(|s| s.scale(c, f)) }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.map(|s| s.mirror(o, n)) }
}

impl<T: SolidStruct, const N: usize> Compound for [T; N] {
	type Elem = T;
	fn clean(&self) -> Result<Self, Error> {
		let v: Result<Vec<T>, Error> = self.iter().map(|s| s.clean()).collect();
		v?.try_into().map_err(|_| unreachable!())
	}
	fn volume(&self) -> f64 { self.iter().map(|s| s.volume()).sum() }
	fn bounding_box(&self) -> [DVec3; 2] {
		self.iter().map(|s| s.bounding_box())
			.reduce(|[amin, amax], [bmin, bmax]| [amin.min(bmin), amax.max(bmax)])
			.unwrap_or([DVec3::ZERO, DVec3::ZERO])
	}
	fn contains(&self, p: DVec3) -> bool { self.iter().any(|s| s.contains(p)) }
	fn shell_count(&self) -> u32 { self.iter().map(|s| s.shell_count()).sum() }
	#[cfg(feature = "color")]
	fn color(self, color: impl Into<Color>) -> Self {
		let c: Color = color.into();
		self.map(|s| s.color(c))
	}
	#[cfg(feature = "color")]
	fn color_clear(self) -> Self {
		self.map(|s| s.color_clear())
	}
	fn union_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_union(self.iter(), tool)
	}
	fn subtract_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_subtract(self.iter(), tool)
	}
	fn intersect_with_metadata<'a>(&self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_intersect(self.iter(), tool)
	}
}

// ==================== impl Wire for Vec<T> / [T; N] ====================
//
// Vec<Edge> is the wire representation in this library — these impls give
// `Vec<Edge>` and `[Edge; N]` the same Wire methods that single Edge has.

impl<T: EdgeStruct> Wire for Vec<T> {
	type Elem = T;

	fn start_point(&self) -> DVec3 {
		self.first().map(|e| e.start_point()).unwrap_or(DVec3::ZERO)
	}

	fn start_tangent(&self) -> DVec3 {
		self.first().map(|e| e.start_tangent()).unwrap_or(DVec3::ZERO)
	}

	fn is_closed(&self) -> bool {
		// Empty wire: not closed. Single-edge wire: defer to that edge.
		// Multi-edge wire: walk the polyline approximation of the last edge to
		// find its end point, and compare with the first edge's start.
		// 1e-6 はモデル単位 (mm) を想定したハードコード — 引数化は API が
		// 増えるため後回し。極小/極大スケールのモデルで誤判定したら直す。
		match self.len() {
			0 => false,
			1 => self[0].is_closed(),
			_ => {
				let start = self[0].start_point();
				let last_pts = self[self.len() - 1].approximation_segments(1e-3);
				let end = last_pts.last().copied().unwrap_or(DVec3::ZERO);
				(start - end).length() < 1e-6
			}
		}
	}

	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {
		let mut out: Vec<DVec3> = Vec::new();
		for e in self {
			let pts = e.approximation_segments(tolerance);
			if let Some((first, rest)) = pts.split_first() {
				if out.last().map(|p| (*p - *first).length() < 1e-9).unwrap_or(false) {
					out.extend_from_slice(rest);
				} else {
					out.push(*first);
					out.extend_from_slice(rest);
				}
			}
		}
		out
	}
}

impl<T: EdgeStruct, const N: usize> Wire for [T; N] {
	type Elem = T;

	fn start_point(&self) -> DVec3 {
		self.first().map(|e| e.start_point()).unwrap_or(DVec3::ZERO)
	}

	fn start_tangent(&self) -> DVec3 {
		self.first().map(|e| e.start_tangent()).unwrap_or(DVec3::ZERO)
	}

	fn is_closed(&self) -> bool {
		match N {
			0 => false,
			1 => self[0].is_closed(),
			_ => {
				let start = self[0].start_point();
				let last_pts = self[N - 1].approximation_segments(1e-3);
				let end = last_pts.last().copied().unwrap_or(DVec3::ZERO);
				(start - end).length() < 1e-6
			}
		}
	}

	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3> {
		let mut out: Vec<DVec3> = Vec::new();
		for e in self {
			let pts = e.approximation_segments(tolerance);
			if let Some((first, rest)) = pts.split_first() {
				if out.last().map(|p| (*p - *first).length() < 1e-9).unwrap_or(false) {
					out.extend_from_slice(rest);
				} else {
					out.push(*first);
					out.extend_from_slice(rest);
				}
			}
		}
		out
	}
}

// ==================== I/O ====================

/// Backend-independent I/O trait.
///
/// `Solid` is an associated type so this trait does not depend on a concrete
/// backend type. Each backend's `Io` impl binds `type Solid = ...;`.
#[allow(non_camel_case_types)]
pub trait IoModule {
	type Solid: SolidStruct;
	fn read_step<R: std::io::Read>(reader: &mut R) -> Result<Vec<Self::Solid>, Error>;
	fn read_brep_binary<R: std::io::Read>(reader: &mut R) -> Result<Vec<Self::Solid>, Error>;
	fn read_brep_text<R: std::io::Read>(reader: &mut R) -> Result<Vec<Self::Solid>, Error>;
	fn write_step<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self::Solid>, writer: &mut W) -> Result<(), Error> where Self::Solid: 'a;
	fn write_brep_binary<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self::Solid>, writer: &mut W) -> Result<(), Error> where Self::Solid: 'a;
	fn write_brep_text<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self::Solid>, writer: &mut W) -> Result<(), Error> where Self::Solid: 'a;
	fn mesh<'a>(solids: impl IntoIterator<Item = &'a Self::Solid>, tolerance: f64) -> Result<Mesh, Error> where Self::Solid: 'a;
}
