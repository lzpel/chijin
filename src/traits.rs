//! Backend-independent trait definitions.
//!
//! - `SolidStruct` (pub(crate)): backend implementation trait.
//!   build_delegation.rs parses this and generates pub inherent methods on Solid.
//!   Trait name follows `<Type>Struct` convention (SolidStruct → Solid).
//!
//! - `SolidExt` (pub): operations available on Solid, Vec<T>, and [T; N].
//!   Not processed by build_delegation. Users import this trait for collection ops.
//!
//! パーサー制約（build_delegation.rs — 行ベースのテキスト処理）:
//! - fn シグネチャは1行に収めること
//! - ライフタイム付きメソッドはスキップされる
//! - #[cfg] は直前1行のみ認識

#[cfg(feature = "color")]
use crate::common::color::Color;
use crate::common::error::Error;
use crate::common::mesh::Mesh;
use crate::{Edge, Face, Solid};
use glam::DVec3;

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
/// build_delegation.rs generates `impl Solid { pub fn ... }` from this trait.
/// Methods with lifetime parameters are skipped by the codegen.
pub trait SolidStruct: Sized + Clone {
	// --- Constructors ---
	fn cube(x: f64, y: f64, z: f64) -> Self;
	fn sphere(radius: f64) -> Self;
	fn cylinder(r: f64, axis: DVec3, h: f64) -> Self;
	fn cone(r1: f64, r2: f64, axis: DVec3, h: f64) -> Self;
	fn torus(r1: f64, r2: f64, axis: DVec3) -> Self;
	fn half_space(plane_origin: DVec3, plane_normal: DVec3) -> Self;

	// --- Transforms ---
	fn translate(self, translation: DVec3) -> Self;
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
	fn rotate_x(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::X, angle) }
	fn rotate_y(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Y, angle) }
	fn rotate_z(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Z, angle) }
	fn scale(self, center: DVec3, factor: f64) -> Self;
	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self;
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
	fn color(self, color: impl Into<Color>) -> Self;
	#[cfg(feature = "color")]
	fn color_clear(self) -> Self;

	// --- Boolean (skipped by build_delegation due to lifetime params) ---
	fn boolean_union<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_subtract<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
	fn boolean_intersect<'a, 'b>(a: impl IntoIterator<Item = &'a Self>, b: impl IntoIterator<Item = &'b Self>) -> Result<(Vec<Self>, [Vec<u64>; 2]), Error> where Self: 'a + 'b;
}

// ==================== SolidExt ====================

/// Public trait: operations on Solid, Vec<Solid>, and [Solid; N].
///
/// Users `use cadrum::SolidExt;` to enable method chaining on collections.
pub trait SolidExt: Sized {
	type Elem: SolidStruct;

	// --- Transforms (-> Self) ---
	fn translate(self, translation: DVec3) -> Self;
	fn rotate(self, axis_origin: DVec3, axis_direction: DVec3, angle: f64) -> Self;
	fn rotate_x(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::X, angle) }
	fn rotate_y(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Y, angle) }
	fn rotate_z(self, angle: f64) -> Self { self.rotate(DVec3::ZERO, DVec3::Z, angle) }
	fn scale(self, center: DVec3, factor: f64) -> Self;
	fn mirror(self, plane_origin: DVec3, plane_normal: DVec3) -> Self;
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

// ==================== impl SolidExt for Solid ====================

impl SolidExt for Solid {
	type Elem = Solid;
	fn translate(self, v: DVec3) -> Self { <Self as SolidStruct>::translate(self, v) }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { <Self as SolidStruct>::rotate(self, o, d, a) }
	fn scale(self, c: DVec3, f: f64) -> Self { <Self as SolidStruct>::scale(self, c, f) }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { <Self as SolidStruct>::mirror(self, o, n) }
	fn clean(&self) -> Result<Self, Error> { <Self as SolidStruct>::clean(self) }
	fn volume(&self) -> f64 { <Self as SolidStruct>::volume(self) }
	fn bounding_box(&self) -> [DVec3; 2] { <Self as SolidStruct>::bounding_box(self) }
	fn contains(&self, p: DVec3) -> bool { <Self as SolidStruct>::contains(self, p) }
	fn shell_count(&self) -> u32 { <Self as SolidStruct>::shell_count(self) }
	#[cfg(feature = "color")]
	fn color(self, color: impl Into<Color>) -> Self { <Self as SolidStruct>::color(self, color) }
	#[cfg(feature = "color")]
	fn color_clear(self) -> Self { <Self as SolidStruct>::color_clear(self) }
	fn union_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		let arr = [self];
		Solid::boolean_union(arr.iter(), tool)
	}
	fn subtract_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		let arr = [self];
		Solid::boolean_subtract(arr.iter(), tool)
	}
	fn intersect_with_metadata<'a>(self, tool: impl IntoIterator<Item = &'a Solid>) -> Result<(Vec<Solid>, [Vec<u64>; 2]), Error> {
		let arr = [self];
		Solid::boolean_intersect(arr.iter(), tool)
	}
}

// ==================== impl SolidExt for Vec<T> ====================

impl<T: SolidStruct> SolidExt for Vec<T> {
	type Elem = T;
	fn translate(self, v: DVec3) -> Self { self.into_iter().map(|s| s.translate(v)).collect() }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.into_iter().map(|s| s.rotate(o, d, a)).collect() }
	fn scale(self, c: DVec3, f: f64) -> Self { self.into_iter().map(|s| s.scale(c, f)).collect() }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.into_iter().map(|s| s.mirror(o, n)).collect() }
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

// ==================== impl SolidExt for [T; N] ====================

impl<T: SolidStruct, const N: usize> SolidExt for [T; N] {
	type Elem = T;
	fn translate(self, v: DVec3) -> Self { self.map(|s| s.translate(v)) }
	fn rotate(self, o: DVec3, d: DVec3, a: f64) -> Self { self.map(|s| s.rotate(o, d, a)) }
	fn scale(self, c: DVec3, f: f64) -> Self { self.map(|s| s.scale(c, f)) }
	fn mirror(self, o: DVec3, n: DVec3) -> Self { self.map(|s| s.mirror(o, n)) }
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

// ==================== Boolean metadata helpers ====================

/// Check if a face came from the tool (b-side) of a boolean operation.
pub fn is_tool_face(metadata: &[Vec<u64>; 2], face: &Face) -> bool {
	metadata[1].contains(&face.tshape_id())
}

/// Check if a face came from the shape (a-side) of a boolean operation.
pub fn is_shape_face(metadata: &[Vec<u64>; 2], face: &Face) -> bool {
	metadata[0].contains(&face.tshape_id())
}

// ==================== I/O ====================

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
	fn write_svg<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, direction: DVec3, tolerance: f64, writer: &mut W) -> Result<(), Error> { writer.write_all(Self::mesh(solids, tolerance)?.to_svg(direction).as_bytes()).map_err(|_| Error::SvgExportFailed) }
	fn write_stl<'a, W: std::io::Write>(solids: impl IntoIterator<Item = &'a Solid>, tolerance: f64, writer: &mut W) -> Result<(), Error> { Self::mesh(solids, tolerance)?.write_stl(writer) }
}
