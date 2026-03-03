use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Required OCC toolkit libraries to link against (OCCT 7.8+ / 7.9.x naming).
/// In OCCT 7.8+: TKSTEP*/TKBinTools/TKShapeUpgrade were reorganized into
/// TKDESTEP/TKBin/TKShHealing respectively.
const OCC_LIBS: &[&str] = &[
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
	"TKDESTEP",    // was TKSTEP + TKSTEP209 + TKSTEPAttr + TKSTEPBase
	"TKService",
];

fn main() {
	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

	let (occt_include, occt_lib_dir) = if cfg!(feature = "bundled") {
		build_occt_from_source(&out_dir, &manifest_dir)
	} else if cfg!(feature = "prebuilt") {
		use_system_occt()
	} else {
		panic!("Either 'bundled' or 'prebuilt' feature must be enabled");
	};

	// Link OCC libraries
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in OCC_LIBS {
		println!("cargo:rustc-link-lib=static={}", lib);
	}
	// XDE (XDE-based STEP with color) requires ApplicationFramework libs.
	// In OCCT 7.9.3, library layout (verified by nm):
	//   TKLCAF   — TDocStd_Document, TDocStd_Application (NewDocument / Close)
	//   TKXCAF   — XCAFApp_Application, XCAFDoc_ColorTool, XCAFDoc_ShapeTool,
	//              XCAFDoc_DocumentTool
	//   TKDESTEP — STEPCAFControl_Reader / Writer (already in OCC_LIBS above)
	// XDE (XDE-based STEP with color) requires ApplicationFramework libs.
	// In OCCT 7.9.3, library layout (verified by nm):
	//   TKLCAF   — TDocStd_Document, TDocStd_Application (NewDocument / Close)
	//   TKXCAF   — XCAFApp_Application, XCAFDoc_ColorTool, XCAFDoc_ShapeTool,
	//              XCAFDoc_DocumentTool
	//   TKCAF    — TNaming_NamedShape, TNaming_Builder (needed by TKXCAF's XCAFDoc)
	//   TKCDF    — CDM_Document, CDM_Application (needed by TKLCAF's TDocStd_Document)
	//   TKDESTEP — STEPCAFControl_Reader / Writer (already in OCC_LIBS above)
	if cfg!(feature = "color") {
		for lib in &["TKLCAF", "TKXCAF", "TKCAF", "TKCDF"] {
			println!("cargo:rustc-link-lib=static={}", lib);
		}
	}

	// Safety-net: suppress any residual duplicate-symbol errors when linking
	// against OCCT static libraries on MinGW.  The primary fix is the
	// OCC_CONVERT_SIGNALS define added below to the cxx_build step.
	println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");

	// TKernel's OSD_WNT.cxx registers a static initialiser (Init_OSD_WNT) that
	// calls advapi32 functions (AllocateAndInitializeSid etc.) at program startup.
	// Standard_Macro.hxx forcibly undefs OCCT_UWP unless WINAPI_FAMILY_APP is set,
	// so the dependency cannot be removed via compiler flags alone.
	// Rust passes -nodefaultlibs, bypassing GCC's spec that normally adds -ladvapi32.
	//
	// Additional Windows system libs required by OCCT static libs:
	//   ole32          — Image_AlienPixMap uses CoInitializeEx / CoCreateInstance /
	//                    CreateStreamOnHGlobal / GetHGlobalFromStream (WIC image I/O)
	//   windowscodecs  — GUID_WICPixelFormat* / CLSID_WICImagingFactory / GUID_ContainerFormat*
	//                    data symbols (Windows Imaging Component GUIDs)
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		println!("cargo:rustc-link-arg=-ladvapi32");
		println!("cargo:rustc-link-arg=-lole32");
		println!("cargo:rustc-link-arg=-lwindowscodecs");
	}

	// Build cxx bridge + C++ wrapper
	let mut bridge_sources: Vec<&str> = vec!["src/ffi.rs"];

	// When the "color" feature is enabled, also compile the color FFI bridge.
	if cfg!(feature = "color") {
		bridge_sources.push("src/color_ffi.rs");
	}

	let mut build = cxx_build::bridges(bridge_sources);

	build
		.file("cpp/wrapper.cpp")
		.include(&occt_include)
		.std("c++17")
		.define("_USE_MATH_DEFINES", None);

	// Define CHIJIN_COLOR for C++ when the "color" feature is enabled.
	if cfg!(feature = "color") {
		build.define("CHIJIN_COLOR", None);
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

	build.compile("chijin_cpp");

	println!("cargo:rerun-if-changed=src/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
	if cfg!(feature = "color") {
		println!("cargo:rerun-if-changed=src/color_ffi.rs");
	}
}

/// Feature "bundled": Download OCCT 7.9.3 source and build with CMake.
fn build_occt_from_source(out_dir: &Path, manifest_dir: &Path) -> (PathBuf, PathBuf) {
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

	// Install into target/occt for a stable, predictable location
	let occt_root = manifest_dir.join("target").join("occt");

	// Determine lib path (CMake on Windows/MinGW installs to win64/gcc/lib)
	let lib_dir = find_occt_lib_dir(&occt_root);

	// Build with CMake only if not already installed
	if !lib_dir.exists() {
		eprintln!("Building OCCT with CMake (this may take a while)...");

		let built = cmake::Config::new(&source_dir)
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
			.define("USE_XLIB", "OFF") // X11 開発パッケージを要求されないようにする
			// Only build the modules we need
			.define("BUILD_MODULE_FoundationClasses", "ON")
			.define("BUILD_MODULE_ModelingData", "ON")
			.define("BUILD_MODULE_ModelingAlgorithms", "ON")
			.define("BUILD_MODULE_DataExchange", "ON")
			.define("BUILD_MODULE_Visualization", "OFF")
			// ApplicationFramework is required for XDE (colored STEP I/O, feature = "color").
		// Enabled unconditionally so the bundled OCCT build includes TKCAF / TKXDESTEP.
		.define("BUILD_MODULE_ApplicationFramework", "ON")
			.define("BUILD_MODULE_Draw", "OFF")
			.define("BUILD_DOC_Overview", "OFF")
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

/// Feature "prebuilt": Use system-installed OCCT.
fn use_system_occt() -> (PathBuf, PathBuf) {
	let occt_root = env::var("OCCT_ROOT")
		.or_else(|_| env::var("CASROOT"))
		.expect(
			"OCCT_ROOT or CASROOT environment variable must be set \
             when using the 'prebuilt' feature",
		);

	let occt_root = PathBuf::from(occt_root);

	let include_dir = find_occt_include_dir(&occt_root);
	let lib_dir = find_occt_lib_dir(&occt_root);

	assert!(
		include_dir.exists(),
		"OCCT include directory not found at {}",
		include_dir.display()
	);
	assert!(
		lib_dir.exists(),
		"OCCT lib directory not found at {}",
		lib_dir.display()
	);

	(include_dir, lib_dir)
}
