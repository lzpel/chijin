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

	let (occt_include, occt_lib_dir) = if cfg!(feature = "buildin") {
		build_occt_from_source(&out_dir, &manifest_dir)
	} else if cfg!(feature = "OCCT_ROOT") {
		use_system_occt()
	} else {
		panic!("Either 'buildin' or 'OCCT_ROOT' feature must be enabled");
	};

	// Link OCC libraries
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in OCC_LIBS {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	// Standard_ErrorHandler inline methods are defined both in the OCCT header
	// (compiled into wrapper.o) and in Standard_ErrorHandler.cxx (in TKernel.a).
	// MinGW's linker treats both as strong symbols and errors; allow the duplicate.
	println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");

	// TKernel's OSD_WNT.cxx registers a static initialiser (Init_OSD_WNT) that
	// calls advapi32 functions (AllocateAndInitializeSid etc.) at program startup.
	// Standard_Macro.hxx forcibly undefs OCCT_UWP unless WINAPI_FAMILY_APP is set,
	// so the dependency cannot be removed via compiler flags alone.
	// Rust passes -nodefaultlibs, bypassing GCC's spec that normally adds -ladvapi32.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		println!("cargo:rustc-link-arg=-ladvapi32");
	}

	// Build cxx bridge + C++ wrapper
	cxx_build::bridge("src/ffi.rs")
		.file("cpp/wrapper.cpp")
		.include(&occt_include)
		.std("c++17")
		.define("_USE_MATH_DEFINES", None)
		.compile("chijin_cpp");

	println!("cargo:rerun-if-changed=src/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Feature "buildin": Download OCCT 7.9.3 source and build with CMake.
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
			// Only build the modules we need
			.define("BUILD_MODULE_FoundationClasses", "ON")
			.define("BUILD_MODULE_ModelingData", "ON")
			.define("BUILD_MODULE_ModelingAlgorithms", "ON")
			.define("BUILD_MODULE_DataExchange", "ON")
			.define("BUILD_MODULE_Visualization", "OFF")
			.define("BUILD_MODULE_ApplicationFramework", "OFF")
			.define("BUILD_MODULE_Draw", "OFF")
			.define("BUILD_DOC_Overview", "OFF")
			.build();

		eprintln!("OCCT built at: {}", built.display());
	}

	// Re-resolve lib dir after build (in case it was just created)
	let lib_dir = find_occt_lib_dir(&occt_root);

	// Determine include path
	let include_dir = if occt_root.join("include").join("opencascade").exists() {
		occt_root.join("include").join("opencascade")
	} else if occt_root.join("inc").exists() {
		occt_root.join("inc")
	} else {
		occt_root.join("include")
	};

	(include_dir, lib_dir)
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

/// Feature "OCCT_ROOT": Use system-installed OCCT.
fn use_system_occt() -> (PathBuf, PathBuf) {
	let occt_root = env::var("OCCT_ROOT")
		.or_else(|_| env::var("CASROOT"))
		.expect(
			"OCCT_ROOT or CASROOT environment variable must be set \
             when using the 'OCCT_ROOT' feature",
		);

	let occt_root = PathBuf::from(occt_root);

	// Try common include paths
	let include_dir = if occt_root.join("include").join("opencascade").exists() {
		occt_root.join("include").join("opencascade")
	} else if occt_root.join("inc").exists() {
		occt_root.join("inc")
	} else {
		occt_root.join("include")
	};

	// Try common lib paths
	let lib_dir = if occt_root.join("win64").join("vc14").join("lib").exists() {
		occt_root.join("win64").join("vc14").join("lib")
	} else {
		occt_root.join("lib")
	};

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
