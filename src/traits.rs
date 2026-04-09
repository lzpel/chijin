//! Backend-independent trait definitions.
//!
//! Trait hierarchy:
//!
//! ```text
//! Transform  ─┐
//!             ├─  SolidExt  ─┐
//!             │              ├─  SolidStruct (pub(crate))
//!             │              │
//!             └──────────────┘
//! ```
//!
//! - `Transform` (pub): spatial ops (translate/rotate/scale/mirror). Geometry-agnostic.
//!   Implemented for shapes (`Solid`, future `Edge` etc.) and collections.
//!
//! - `SolidExt: Transform` (pub): solid-specific operations on Solid, Vec<T>, and [T; N]
//!   (clean/volume/contains/color/boolean wrappers). Inherits Transform's methods.
//!
//! - `SolidStruct: Sized + Clone + SolidExt` (pub(crate)): backend implementation trait.
//!   Adds Solid-only operations (constructors, topology accessors, boolean primitives).
//!   build_delegation.rs parses this and generates pub inherent methods on Solid,
//!   walking the supertrait chain so all `SolidExt` and `Transform` methods are also
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
//!   FaceStruct       ← 単独。下位を一切知らない
//!   EdgeStruct       ← 単独。下位を一切知らない
//!   SolidStruct      ← type Edge: EdgeStruct;  type Face: FaceStruct;
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
/// shapes (`Solid`, eventually `Edge` etc.) and for collections (`Vec<T>`,
/// `[T; N]`) where the element type is itself `Transform`.
///
/// `SolidExt: Transform`, so users of `Solid` get these methods inherently
/// (via build_delegation's supertrait walk) and never need to import this trait
/// explicitly. Importing it is only required when calling these methods on
/// `Vec<T>` / `[T; N]` directly.
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

// ==================== Per-type traits ====================

/// Backend-independent face trait.
pub trait FaceStruct {
	fn normal_at_center(&self) -> DVec3;
	fn center_of_mass(&self) -> DVec3;
}

/// Backend-independent edge trait.
pub trait EdgeStruct {
	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3>;
}

/// Backend-independent solid trait (pub(crate) — not exposed to users).
///
/// `Solid`-specific operations only. The shared methods (transforms, queries,
/// color, boolean wrappers) live on `SolidExt` and are inherited via the
/// supertrait bound.
///
/// build_delegation.rs generates `impl Solid { pub fn ... }` from this trait
/// and walks the supertrait chain to expose `SolidExt` methods inherently as well.
///
/// Associated types `Edge`/`Face` keep this trait backend-independent: each
/// backend (occt / pure) binds them to its own concrete types in the impl.
pub trait SolidStruct: Sized + Clone + SolidExt {
	type Edge: EdgeStruct;
	type Face: FaceStruct;

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

	// --- Boolean primitives (consumed by SolidExt::*_with_metadata wrappers) ---
	fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
}

// ==================== SolidExt ====================

/// Public trait: solid-specific operations on Solid, Vec<Solid>, and [Solid; N].
///
/// Spatial transforms (translate/rotate/scale/mirror) live on the supertrait
/// `Transform`. Users `use cadrum::SolidExt;` (and optionally `Transform`) to
/// enable method chaining on collections.
pub trait SolidExt: Transform {
	type Elem: SolidStruct;

	fn clean(&self) -> Result<Self, Error>;

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
	fn union_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn subtract_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn intersect_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<(Vec<Self::Elem>, [Vec<u64>; 2]), Error> where Self::Elem: 'a;
	fn union<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.union_with_metadata(tool)?.0) }
	fn subtract<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.subtract_with_metadata(tool)?.0) }
	fn intersect<'a>(self, tool: impl IntoIterator<Item = &'a Self::Elem>) -> Result<Vec<Self::Elem>, Error> where Self::Elem: 'a { Ok(self.intersect_with_metadata(tool)?.0) }
}

// `impl SolidExt for Solid` lives in the backend module (e.g. src/occt/solid.rs)
// because it needs direct access to the backend FFI for the per-element operations.

// ==================== impl Transform / SolidExt for Vec<T> ====================

impl<T: Transform> Transform for Vec<T> {
	fn translate(self, v: DVec3) -> Self { self.into_iter().map(|s| s.translate(v)).collect() }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.into_iter().map(|s| s.rotate(o, d, a)).collect() }
	fn scale(self, c: DVec3, f: f64) -> Self { self.into_iter().map(|s| s.scale(c, f)).collect() }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.into_iter().map(|s| s.mirror(o, n)).collect() }
}

impl<T: SolidStruct> SolidExt for Vec<T> {
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
	fn union_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_union(self.iter(), tool)
	}
	fn subtract_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_subtract(self.iter(), tool)
	}
	fn intersect_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_intersect(self.iter(), tool)
	}
}

// ==================== impl Transform / SolidExt for [T; N] ====================

impl<T: Transform, const N: usize> Transform for [T; N] {
	fn translate(self, v: DVec3) -> Self { self.map(|s| s.translate(v)) }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.map(|s| s.rotate(o, d, a)) }
	fn scale(self, c: DVec3, f: f64) -> Self { self.map(|s| s.scale(c, f)) }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.map(|s| s.mirror(o, n)) }
}

impl<T: SolidStruct, const N: usize> SolidExt for [T; N] {
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
	fn union_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_union(self.iter(), tool)
	}
	fn subtract_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_subtract(self.iter(), tool)
	}
	fn intersect_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a T>) -> Result<(Vec<T>, [Vec<u64>; 2]), Error> where T: 'a {
		T::boolean_intersect(self.iter(), tool)
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
	fn write_svg<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self::Solid>, direction: DVec3, tolerance: f64, writer: &mut W) -> Result<(), Error> where Self::Solid: 'a { writer.write_all(Self::mesh(solids, tolerance)?.to_svg(direction).as_bytes()).map_err(|_| Error::SvgExportFailed) }
	fn write_stl<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Self::Solid>, tolerance: f64, writer: &mut W) -> Result<(), Error> where Self::Solid: 'a { Self::mesh(solids, tolerance)?.write_stl(writer) }
}
