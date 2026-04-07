//! Backend-independent trait definitions (pub(crate) — not exposed to users).
//! バックエンド共通のトレイト定義（pub(crate) — ユーザーに非公開）。
//!
//! build_delegation.rs parses this file and auto-generates pub inherent methods
//! on concrete types (Solid, Face, Edge). Trait names must follow the `<Type>Trait`
//! convention (e.g. SolidTrait → Solid).
//!
//! build_delegation.rs がこのファイルをパースして、具象型 (Solid, Face, Edge) の
//! pub inherent methods を自動生成する。トレイト名は `<Type>Trait` 規則に従うこと。
//!
//! Constraints (build_delegation.rs uses a line-based text parser):
//! パーサー制約（行ベースのテキスト処理）:
//! - fn signatures must fit on a single line / fn シグネチャは1行に収めること
//! - lifetime parameters and where clauses are not supported / ライフタイム・where句は非対応
//! - #[cfg(...)] is recognized only on the immediately preceding line / #[cfg] は直前1行のみ認識

#[cfg(feature = "color")]
use crate::common::color::Color;
use crate::common::error::Error;
use crate::common::mesh::Mesh;
use crate::{Edge, Face, Solid};
use glam::DVec3;

/// Backend-independent face trait.
pub trait FaceTrait {
	fn normal_at_center(&self) -> DVec3;
	fn center_of_mass(&self) -> DVec3;
}

/// Backend-independent edge trait.
pub trait EdgeTrait {
	fn approximation_segments(&self, tolerance: f64) -> Vec<DVec3>;
}

/// Backend-independent solid trait.
///
/// Defines the common interface that both OCCT and Pure Rust backends must implement.
pub trait SolidTrait: Sized + Clone {
	// --- Constructors ---
	fn cube(x: f64, y: f64, z: f64) -> Self;
	fn sphere(center: DVec3, radius: f64) -> Self;
	fn cylinder(p: DVec3, r: f64, dir: DVec3, h: f64) -> Self;
	fn cone(p: DVec3, dir: DVec3, r1: f64, r2: f64, h: f64) -> Self;
	fn torus(p: DVec3, dir: DVec3, r1: f64, r2: f64) -> Self;
	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Self;

	// --- Transforms ---
	fn translate(self, translation: DVec3) -> Self;
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
	fn scaled(&self, center: DVec3, factor: f64) -> Self;
	fn mirrored(&self, plane_origin: DVec3, plane_normal: DVec3) -> Self;
	fn clean(&self) -> Result<Self, Error>;

	// --- Queries ---
	fn volume(&self) -> f64;
	fn bounding_box(&self) -> [DVec3; 2];
	fn contains(&self, point: DVec3) -> bool;
	fn shell_count(&self) -> u32;

	// --- Topology ---
	fn faces(&self) -> Vec<Face>;
	fn edges(&self) -> Vec<Edge>;

	// --- Color ---
	#[cfg(feature = "color")]
	fn color_paint(self, color: Option<Color>) -> Self;
	#[cfg(feature = "color")]
	fn color(&self) -> Option<Color>;
}

/// Backend-independent boolean operation trait.
pub trait BooleanTrait: Sized {
	fn union<'a>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'a Solid>) -> Result<Self, Error>;
	fn subtract<'a>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'a Solid>) -> Result<Self, Error>;
	fn intersect<'a>(a: impl IntoIterator<Item = &'a Solid>, b: impl IntoIterator<Item = &'a Solid>) -> Result<Self, Error>;
	fn is_tool_face(&self, face: &Face) -> bool;
	fn is_shape_face(&self, face: &Face) -> bool;
	fn solids(&self) -> &[Solid];
	fn into_solids(self) -> Vec<Solid>;
}

/// Backend-independent I/O trait.
#[allow(non_camel_case_types)]
pub trait IoModule {
	fn read_step<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error>;
	fn read_brep_binary<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error>;
	fn read_brep_text<R: std::io::Read>(reader: &mut R) -> Result<Vec<Solid>, Error>;
	fn write_step<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error>;
	fn write_brep_binary<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error>;
	fn write_brep_text<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, writer: &mut W) -> Result<(), Error>;
	fn mesh<'a>(solids: impl IntoIterator<Item = &'a Solid>, tolerance: f64) -> Result<Mesh, Error>;
	fn write_svg<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, direction: DVec3, tolerance: f64, writer: &mut W) -> Result<(), Error>;
}
