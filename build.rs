mod build_delegation;

use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=OCCT_ROOT");
	println!("cargo:rerun-if-changed=src/traits.rs");

	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	build_delegation::build_delegation(include_str!("src/traits.rs"), &out_dir);

	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

	let occt_root = env::var("OCCT_ROOT")
		.map(PathBuf::from)
		.unwrap_or_else(|_| manifest_dir.join("target").join("occt"));

	let lib_dir = find_occt_lib_dir(&occt_root);
	println!("cargo:rerun-if-changed={}", lib_dir.display());
	let (occt_include, occt_lib_dir) = if lib_dir.exists() {
		// Libraries found — link only, no rebuild
		(find_occt_include_dir(&occt_root), lib_dir)
	} else {
		// No libraries found — build OCCT from source (this may take 10-30 minutes)
		eprintln!("cargo:warning=OCCT not found at {}. Building from source — this may take 10-30 minutes.", occt_root.display());
		build_occt_from_source(&out_dir, &occt_root)
	};

	link_occt_libraries(&occt_include, &occt_lib_dir, cfg!(feature = "color"));
}

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path, color: bool) {
	// Required OCC toolkit libraries to link against (OCCT 7.8+ / 7.9.x naming).
	// In OCCT 7.8+: TKSTEP*/TKBinTools/TKShapeUpgrade were reorganized into
	// TKDESTEP/TKBin/TKShHealing respectively.
	let occ_libs = &[
		"TKernel",
		"TKMath",
		"TKBRep",
		"TKTopAlgo",
		"TKPrim",
		"TKBO",
		"TKBool",
		"TKShHealing", // includes former TKShapeUpgrade
		"TKMesh",
		"TKGeomBase",
		"TKGeomAlgo",
		"TKG3d",
		"TKG2d",
		"TKBin", // was TKBinTools
		"TKXSBase",
		"TKDE",        // DE framework base (OCCT 7.8+)
		"TKDECascade", // DE cascade bridge (OCCT 7.8+)
		"TKOffset",    // BRepOffsetAPI_MakePipeShell (helix sweep)
		"TKDESTEP",    // was TKSTEP + TKSTEP209 + TKSTEPAttr + TKSTEPBase
		               // TKService is NOT linked here: it contains Image_AlienPixMap (WIC image I/O)
		               // which pulls in ole32/windowscodecs on Windows, but image I/O is unused in
		               // the base API.  TKService is added below only when "color" is enabled because
		               // TKXCAF references Graphic3d_* symbols that live in TKService.
	];

	// Link OCC libraries
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in occ_libs {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// XDE (XDE-based STEP with color) requires ApplicationFramework libs.
	// In OCCT 7.9.3, library layout (verified by nm):
	//   TKLCAF   — TDocStd_Document, TDocStd_Application (NewDocument / Close)
	//   TKXCAF   — XCAFApp_Application, XCAFDoc_ColorTool, XCAFDoc_ShapeTool,
	//              XCAFDoc_DocumentTool
	//   TKCAF    — TNaming_NamedShape, TNaming_Builder (needed by TKXCAF's XCAFDoc)
	//   TKCDF    — CDM_Document, CDM_Application (needed by TKLCAF's TDocStd_Document)
	//   TKDESTEP — STEPCAFControl_Reader / Writer (already in OCC_LIBS above)
	//   TKService — Graphic3d_* symbols referenced by TKXCAF (XCAFDoc_VisMaterial etc.)
	if color {
		for lib in &["TKLCAF", "TKXCAF", "TKCAF", "TKCDF"] {
			println!("cargo:rustc-link-lib=static={}", lib);
		}
	}

	// Safety-net: suppress any residual duplicate-symbol errors when linking
	// against OCCT static libraries on MinGW.  The primary fix is the
	// OCC_CONVERT_SIGNALS define added below to the cxx_build step.
	// Guard to GNU only: -Wl,... is GCC/ld syntax and is invalid on MSVC link.exe.
	if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	// TKernel's OSD_WNT.cxx registers a static initialiser (Init_OSD_WNT) that
	// calls advapi32 functions (AllocateAndInitializeSid etc.) at program startup.
	// Standard_Macro.hxx forcibly undefs OCCT_UWP unless WINAPI_FAMILY_APP is set,
	// so the dependency cannot be removed via compiler flags alone.
	// Rust passes -nodefaultlibs, bypassing GCC's spec that normally adds -ladvapi32.
	//
	// Additional Windows system libs required by OCCT static libs (color feature only):
	//   ole32         — Image_AlienPixMap uses CoInitializeEx / CoCreateInstance /
	//                   CreateStreamOnHGlobal / GetHGlobalFromStream (WIC image I/O)
	//   windowscodecs — GUID_WICPixelFormat* / CLSID_WICImagingFactory data symbols
	//                   (pulled in transitively via TKService → Image_AlienPixMap)
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		// Use rustc-link-lib (not rustc-link-arg) so Cargo translates correctly
		// for both toolchains: MSVC gets advapi32.lib, GNU gets -ladvapi32.
		println!("cargo:rustc-link-lib=advapi32");
		// OSD_signal.cxx (TKernel) uses MessageBoxA / MessageBeep on MSVC Windows
		// (#if !defined(__MINGW32__) && !defined(__CYGWIN32__) block).
		// MinGW links user32 implicitly; MSVC requires explicit declaration.
		println!("cargo:rustc-link-lib=user32");
	}

	// Build cxx bridge + C++ wrapper
	let mut build = cxx_build::bridge("src/occt/ffi.rs");
	build
		.file("cpp/wrapper.cpp")
		.include(occt_include)
		.std("c++17")
		.define("_USE_MATH_DEFINES", None);

	// Define CADRUM_COLOR for C++ when the "color" feature is enabled.
	if color {
		build.define("CADRUM_COLOR", None);
	}

	// On MinGW (Windows GNU toolchain), GCC at -O0 emits inline C++ methods
	// (from Standard_ErrorHandler.hxx) as strong (non-COMDAT) symbols in wrapper.o.
	// TKernel.a unconditionally defines the same methods in Standard_ErrorHandler.cxx.obj.
	// This causes "multiple definition" link errors.
	//
	// Standard_ErrorHandler.hxx only generates the inline stubs when OCC_CONVERT_SIGNALS
	// is NOT defined (via `#if !defined(OCC_CONVERT_SIGNALS)`).  Defining it here
	// suppresses those stubs in wrapper.o so only TKernel.a's implementation is linked.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
		&& env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
	{
		build.define("OCC_CONVERT_SIGNALS", None);
	}

	build.compile("cadrum_cpp");

	println!("cargo:rerun-if-changed=src/occt/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Download OCCT 7.9.3 source, patch, and build with CMake into `install_prefix`.
fn build_occt_from_source(out_dir: &Path, install_prefix: &Path) -> (PathBuf, PathBuf) {
	let occt_version = "V7_9_3";
	let occt_url = format!(
		"https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz",
		occt_version
	);

	let download_dir = out_dir.join("occt-source");

	// Use a sentinel file to track successful extraction.
	let extraction_sentinel = download_dir.join(".extraction_done");

	if !extraction_sentinel.exists() {
		std::fs::create_dir_all(&download_dir).unwrap();

		// Clean up any partial extraction from a previous failed attempt
		if let Ok(entries) = std::fs::read_dir(&download_dir) {
			for entry in entries.flatten() {
				let name = entry.file_name();
				if name.to_string_lossy().starts_with("OCCT") && entry.path().is_dir() {
					eprintln!("Removing partial OCCT extraction: {:?}", name);
					let _ = std::fs::remove_dir_all(entry.path());
				}
			}
		}

		eprintln!("Downloading OCCT {} ...", occt_version);

		// Download using ureq (pure Rust HTTP client)
		let response = ureq::get(&occt_url)
			.call()
			.expect("Failed to download OCCT source tarball");

		let mut body = Vec::new();
		response
			.into_body()
			.into_reader()
			.read_to_end(&mut body)
			.expect("Failed to read OCCT download response body");

		eprintln!("Downloaded {} bytes. Extracting...", body.len());

		// Extract using libflate + tar (pure Rust)
		let gz_decoder =
			libflate::gzip::Decoder::new(&body[..]).expect("Failed to initialize gzip decoder");
		let mut archive = tar::Archive::new(gz_decoder);
		archive
			.unpack(&download_dir)
			.expect("Failed to extract OCCT source tarball");

		// Write sentinel to mark successful extraction
		std::fs::write(&extraction_sentinel, "done").unwrap();
		eprintln!("OCCT source extracted successfully.");
	}

	// Auto-detect the extracted OCCT directory name
	// (GitHub archives may name it OCCT-V7_9_3 or OCCT-7_9_3 depending on the tag)
	let source_dir = std::fs::read_dir(&download_dir)
		.expect("Failed to read occt-source directory")
		.flatten()
		.find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir())
		.map(|e| e.path())
		.expect("OCCT source directory not found after extraction");

	// Patch OCCT sources to remove TKService (Visualization) dependencies.
	// XCAFDoc_VisMaterial.cxx and XCAFPrs_Texture.cxx reference Graphic3d_* symbols
	// that live in TKService, which we don't build (BUILD_MODULE_Visualization=OFF).
	// The non-visualization TDF_Attribute methods (GetID, Restore, Paste, …) are
	// kept intact; only FillMaterialAspect / FillAspect are emptied.
	patch_occt_sources(&source_dir);

	let occt_root = install_prefix;

	// Determine lib path (CMake on Windows/MinGW installs to win64/gcc/lib)
	let lib_dir = find_occt_lib_dir(&occt_root);

	// Build with CMake only if not already installed
	if !lib_dir.exists() {
		eprintln!("Building OCCT with CMake (this may take a while)...");

		let built = cmake::Config::new(&source_dir)
			.profile("Release")
			.define("BUILD_LIBRARY_TYPE", "Static")
			.define("CMAKE_INSTALL_PREFIX", occt_root.to_str().unwrap())
			// Disable optional dependencies we don't need
			.define("USE_FREETYPE", "OFF")
			.define("USE_FREEIMAGE", "OFF")
			.define("USE_OPENVR", "OFF")
			.define("USE_FFMPEG", "OFF")
			.define("USE_TBB", "OFF")
			.define("USE_VTK", "OFF")
			.define("USE_RAPIDJSON", "OFF")
			.define("USE_DRACO", "OFF")
			.define("USE_TK", "OFF")
			.define("USE_TCL", "OFF")
			.define("USE_XLIB", "OFF")
			.define("USE_OPENGL", "OFF")
			.define("USE_GLES2", "OFF")
			.define("USE_EGL", "OFF")
			.define("USE_D3D", "OFF")
			// Only build the modules we need
			.define("BUILD_MODULE_FoundationClasses", "ON")
			.define("BUILD_MODULE_ModelingData", "ON")
			.define("BUILD_MODULE_ModelingAlgorithms", "ON")
			.define("BUILD_MODULE_DataExchange", "ON")
			.define("BUILD_MODULE_Visualization", "OFF")
			.define("BUILD_MODULE_ApplicationFramework", "OFF")
			.define("BUILD_MODULE_Draw", "OFF")
			.define("BUILD_DOC_Overview", "OFF")
			.define("BUILD_DOC_RefMan", "OFF")
			.define("BUILD_YACCLEX", "OFF")
			.define("BUILD_RESOURCES", "OFF")
			.define("BUILD_SAMPLES_MFC", "OFF")
			.define("BUILD_SAMPLES_QT", "OFF")
			.define("BUILD_Inspector", "OFF")
			.define("BUILD_ENABLE_FPE_SIGNAL_HANDLER", "OFF")
			.build();

		eprintln!("OCCT built at: {}", built.display());
	}

	// Re-resolve lib dir after build (in case it was just created)
	let lib_dir = find_occt_lib_dir(&occt_root);
	let include_dir = find_occt_include_dir(&occt_root);

	(include_dir, lib_dir)
}

/// Find the OCCT include directory, checking common install layouts.
fn find_occt_include_dir(occt_root: &Path) -> PathBuf {
	let candidates = [
		occt_root.join("include").join("opencascade"),
		occt_root.join("inc"),
		occt_root.join("include"),
	];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("include")
}

/// Find the OCCT lib directory, checking common install layouts.
/// CMake on Windows/MinGW installs to win64/gcc/lib; on Linux to lib.
fn find_occt_lib_dir(occt_root: &Path) -> PathBuf {
	let candidates = [
		occt_root.join("lib"),
		occt_root.join("win64").join("gcc").join("lib"),
		occt_root.join("win64").join("vc14").join("lib"),
	];
	for dir in &candidates {
		if dir.exists() {
			return dir.clone();
		}
	}
	// Default fallback
	occt_root.join("lib")
}

/// Patch two OCCT source files that pull in Graphic3d_* (TKService) symbols even
/// when BUILD_MODULE_Visualization=OFF:
///
///  - XCAFDoc/XCAFDoc_VisMaterial.cxx: remove #include lines for Graphic3d_Aspects,
///    Graphic3d_MaterialAspect and XCAFPrs_Texture, then empty the bodies of
///    FillMaterialAspect() and FillAspect() — the only methods that use those types.
///    All TDF_Attribute interface methods (GetID, Restore, Paste, …) are left intact.
///
///  - XCAFPrs/XCAFPrs_Texture.cxx: replaced with an empty file because it defines
///    XCAFPrs_Texture which inherits from Graphic3d_Texture2D (TKService).
///    The only caller was FillAspect(), which is now empty.
fn patch_occt_sources(source_dir: &Path) {
	// Stub method bodies only: keep #includes and signatures, empty the bodies.
	stub_out_methods(
		&source_dir.join("src/XCAFDoc/XCAFDoc_VisMaterial.cxx"),
		true,
	);
	// Empty the entire file: the initializer list references the base class, so
	// body stubs alone cannot cut the TKService dependency.
	stub_out_methods(&source_dir.join("src/XCAFPrs/XCAFPrs_Texture.cxx"), false);
}

/// Neutralize a C++ source file at `path`.
///
/// # Arguments
/// - `keep_signatures` — `true`: keep `#include`s and signatures, replace only the
///   top-level method bodies with empty stubs. Use when the signature types are still
///   needed by the compiler.
///   `false`: empty the entire file (header comment only). Use when the initializer
///   list references a base class and body stubs alone cannot cut the dependency.
///
/// Stub body rules:
/// - `void` return / constructor / destructor → `{}`
/// - anything else → `{ return {}; }` (value-initialize)
///
/// # Note
/// `keep_signatures: true` cannot be used on files that have top-level
/// `namespace {}` or `extern "C" {}` blocks. Intended for `.cxx` implementation files only.
fn stub_out_methods(path: &Path, keep_signatures: bool) {
	if !path.exists() {
		return;
	}

	// Build the header comment that records the stub operation.
	let timestamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| {
			let secs = d.as_secs();
			let (h, mi, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
			format!("{:02}:{:02}:{:02} UTC", h, mi, s)
		})
		.unwrap_or_else(|_| "unknown".to_string());
	let description = if keep_signatures {
		"all method bodies replaced with empty stubs"
	} else {
		"file emptied"
	};
	let header = format!(
		"// Stubbed by cadrum build.rs: {}.\n\
		 // Stubbed at: {}\n",
		description, timestamp
	);

	let patched = if keep_signatures {
		let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
		// Replace all top-level brace blocks with empty stubs.
		header + &stub_all_top_level_bodies(&content)
	} else {
		// Empty the file (header comment only).
		header
	};

	std::fs::write(path, patched).expect("Failed to write stubbed file");
	eprintln!("Stubbed {}", path.file_name().unwrap().to_string_lossy());
}

/// Replace every top-level (brace depth 0) `{ … }` block in `content` with
/// `{}` or `{ return {}; }` and return the result.
fn stub_all_top_level_bodies(content: &str) -> String {
	let bytes = content.as_bytes();
	let mut result = String::new();
	let mut depth = 0usize;
	let mut i = 0;
	let mut last_end = 0;

	while i < bytes.len() {
		match bytes[i] {
			b'{' if depth == 0 => {
				// Top-level block start: check the preceding signature for return type.
				let prefix = &content[last_end..i];
				let stub_body = if is_void_return(prefix) {
					"{}"
				} else {
					"{ return {}; }"
				};

				// Walk forward with brace counting to find the matching closing brace.
				depth = 1;
				i += 1;
				while i < bytes.len() && depth > 0 {
					match bytes[i] {
						b'{' => depth += 1,
						b'}' => depth -= 1,
						_ => {}
					}
					i += 1;
				}
				// i now points one past the closing '}'.
				result.push_str(prefix);
				result.push_str(stub_body);
				last_end = i;
				continue;
			}
			b'{' => depth += 1,
			b'}' => {
				if depth > 0 {
					depth -= 1;
				}
			}
			_ => {}
		}
		i += 1;
	}
	result.push_str(&content[last_end..]);
	result
}

/// Return `true` if the signature string indicates the body should be stubbed as `{}`.
///
/// Returns `true` for any of:
/// 1. `void` return type — the identifier "void" appears in the signature
/// 2. Destructor — signature contains `::`~`
/// 3. Constructor — `ClassName::ClassName(` pattern (identifier before and after `::` match)
///
/// Returns `false` otherwise → stub as `{ return {}; }` (value-initialize).
fn is_void_return(prefix: &str) -> bool {
	// Only examine the signature after the last definition terminator (';' or '}').
	let sig = prefix
		.rfind(|c| c == ';' || c == '}')
		.map(|p| &prefix[p + 1..])
		.unwrap_or(prefix);

	// 1. void return type
	if sig
		.split(|c: char| !c.is_alphanumeric() && c != '_')
		.any(|w| w == "void")
	{
		return true;
	}

	// 2. Destructor: contains ::~
	if sig.contains("::~") {
		return true;
	}

	// 3. Constructor: ClassName::ClassName( pattern
	//    Look at everything before '(' and compare the identifiers around the last '::'.`
	if let Some(paren) = sig.find('(') {
		let before_paren = sig[..paren].trim_end();
		if let Some(dc) = before_paren.rfind("::") {
			let method_name = before_paren[dc + 2..].trim();
			let class_name = before_paren[..dc]
				.split(|c: char| !c.is_alphanumeric() && c != '_')
				.filter(|s| !s.is_empty())
				.last()
				.unwrap_or("");
			if !method_name.is_empty() && method_name == class_name {
				return true;
			}
		}
	}

	false
}
