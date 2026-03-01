use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Required OCC toolkit libraries to link against.
const OCC_LIBS: &[&str] = &[
	"TKernel",
	"TKMath",
	"TKBRep",
	"TKTopAlgo",
	"TKPrim",
	"TKBO",
	"TKShHealing",
	"TKShapeUpgrade",
	"TKSTEP",
	"TKSTEP209",
	"TKSTEPAttr",
	"TKSTEPBase",
	"TKMesh",
	"TKGeomBase",
	"TKGeomAlgo",
	"TKG3d",
	"TKG2d",
	"TKBinTools",
	"TKXSBase",
	"TKService",
];

fn main() {
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	let (occt_include, occt_lib_dir) = if cfg!(feature = "buildin") {
		build_occt_from_source(&out_dir)
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

	// Build cxx bridge + C++ wrapper
	cxx_build::bridge("src/ffi.rs")
		.file("cpp/wrapper.cpp")
		.include(&occt_include)
		.std("c++17")
		.compile("chijin_cpp");

	println!("cargo:rerun-if-changed=src/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Feature "buildin": Download OCCT 7.9.3 source and build with CMake.
fn build_occt_from_source(out_dir: &Path) -> (PathBuf, PathBuf) {
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
	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
	let occt_root = manifest_dir.join("target").join("occt");

	// Build with CMake only if not already installed
	if !occt_root.join("lib").exists() {
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

	// Determine include and lib paths
	let include_dir = if occt_root.join("include").join("opencascade").exists() {
		occt_root.join("include").join("opencascade")
	} else if occt_root.join("inc").exists() {
		occt_root.join("inc")
	} else {
		occt_root.join("include")
	};
	let lib_dir = occt_root.join("lib");

	(include_dir, lib_dir)
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
