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

use glam::DVec3;
use crate::common::mesh::Mesh;
use crate::common::error::Error;
#[cfg(feature = "color")]
use crate::common::color::Color;

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
	type Face: FaceTrait;
	type Edge: EdgeTrait;

	// --- Constructors ---
	fn box_from_corners(corner_1: DVec3, corner_2: DVec3) -> Self;
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
	fn faces(&self) -> Vec<Self::Face>;
	fn edges(&self) -> Vec<Self::Edge>;

	// --- Mesh ---
	fn mesh_with_tolerance(&self, tol: f64) -> Result<Mesh, Error>;

	// --- Color ---
	#[cfg(feature = "color")]
	fn color_paint(self, color: Option<Color>) -> Self;
	#[cfg(feature = "color")]
	fn color(&self) -> Option<Color>;
}
