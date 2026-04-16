mod build_delegation;

use std::env;
use std::path::{Path, PathBuf};

/// OCCT release used by cadrum. Update this tag when bumping OCCT versions;
/// `slug()` derives the lowercase/underscore-stripped form used in filenames.
const OCCT_VERSION: &str = "V8_0_0_rc5";

/// GitHub Release tag under `lzpel/cadrum` that hosts the prebuilt tarballs.
/// Bump this when rebuilding prebuilts for the same OCCT version.
#[cfg(not(feature = "source-build"))]
const OCCT_PREBUILT_TAG: &str = "occt-v800rc5";

/// `V8_0_0_rc5` → `v800rc5`. Shared rule: lowercase and drop underscores.
fn slug(version: &str) -> String {
	version.to_ascii_lowercase().replace('_', "")
}

fn main() {
	println!("cargo:rerun-if-env-changed=OCCT_ROOT");
	println!("cargo:rerun-if-env-changed=CADRUM_PREBUILT_URL");
	println!("cargo:rerun-if-changed=src/traits.rs");
	println!("cargo:rerun-if-changed=build_delegation.rs");

	if env::var("DOCS_RS").is_ok() {
		return;
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	build_delegation::build_delegation(include_str!("src/traits.rs"), &out_dir);

	let target = env::var("TARGET").unwrap();

	let target_dir = target_dir_from_out_dir(&out_dir, &target);
	let default_root = target_dir.join(format!("cadrum-occt-{}-{}", slug(OCCT_VERSION), &target));
	let effective_root = env::var("OCCT_ROOT")
		.map(|r| {
			let p = PathBuf::from(r);
			if p.is_relative() { env::current_dir().unwrap().join(p) } else { p }
		})
		.unwrap_or(default_root);

	let [occt_include, occt_lib_dir] = resolve_occt(&effective_root, &target);

	link_occt_libraries(&occt_include, &occt_lib_dir);
}

/// Derive the cargo target directory from `OUT_DIR`.
///
/// `OUT_DIR` layout:
///   `<target_dir>/<profile>/build/<pkg>-<hash>/out`            (no `--target`)
///   `<target_dir>/<triple>/<profile>/build/<pkg>-<hash>/out`   (with `--target`)
fn target_dir_from_out_dir(out_dir: &Path, target: &str) -> PathBuf {
	let above_profile = out_dir.ancestors().nth(4).expect("unexpected OUT_DIR layout");
	if above_profile.file_name().map_or(false, |n| n == target) {
		above_profile.parent().unwrap().to_path_buf()
	} else {
		above_profile.to_path_buf()
	}
}

/// Resolve `[include_dir, lib_dir]` for OCCT.
///
///   1. Cache hit → use it
///   2. Cache miss + `source-build` → build from upstream sources
///   3. Cache miss otherwise → download prebuilt tarball
fn resolve_occt(effective_root: &Path, target: &str) -> [PathBuf; 2] {
	let _ = target; // used only in #[cfg(not(feature = "source-build"))]
	println!("cargo:rerun-if-changed={}", effective_root.display());

	match find_occt_dirs(effective_root) {
		Some(dirs) => return dirs,
		None => {
			#[cfg(feature = "source-build")]
			{
				eprintln!("cargo:warning=OCCT cache miss at {} — building from source (this may take 10-30 minutes)", effective_root.display());
				return source::build_from_source(effective_root)
					.expect("Failed to build OCCT from source");
			}
			#[cfg(not(feature = "source-build"))]
			{
				return download_prebuilt(effective_root, target)
					.unwrap_or_else(|| panic!(
						"\nFailed to download prebuilt OCCT for target `{}`.\n\
						 See README for the list of supported prebuilt targets, or enable\n\
						 the `source-build` feature to build OCCT from upstream sources:\n\
						 \n    cargo build --features source-build\n",
						target
					));
			}
		}
	}
}

/// Probe `occt_root` for include and lib directories.
/// Returns `Some([include_dir, lib_dir])` if both exist, `None` otherwise.
fn find_occt_dirs(occt_root: &Path) -> Option<[PathBuf; 2]> {
	let pick = |cands: &[PathBuf]| cands.iter().find(|p| p.exists()).cloned();
	let inc = pick(&[occt_root.join("include").join("opencascade"), occt_root.join("inc"), occt_root.join("include")])?;
	let lib = pick(&[occt_root.join("lib"), occt_root.join("win64").join("gcc").join("lib"), occt_root.join("win64").join("clang").join("lib"), occt_root.join("win64").join("vc14").join("lib")])?;
	Some([inc, lib])
}

/// OCCT toolkits to link against (OCCT 7.8+ / 8.x naming).
const OCC_LIBS: &[&str] = &[
	"TKernel", "TKMath", "TKBRep", "TKTopAlgo", "TKPrim", "TKBO", "TKBool",
	"TKShHealing", "TKMesh", "TKGeomBase", "TKGeomAlgo", "TKG3d", "TKG2d",
	"TKBin", "TKXSBase", "TKDE", "TKDECascade", "TKOffset", "TKDESTEP",
	#[cfg(feature = "color")] "TKLCAF",
	#[cfg(feature = "color")] "TKXCAF",
	#[cfg(feature = "color")] "TKCAF",
	#[cfg(feature = "color")] "TKCDF",
];

fn link_occt_libraries(occt_include: &Path, occt_lib_dir: &Path) {
	println!("cargo:rustc-link-search=native={}", occt_lib_dir.display());
	for lib in OCC_LIBS {
		println!("cargo:rustc-link-lib=static={}", lib);
	}

	let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
	let is_mingw_like = target_env == "gnu" || target_env == "gnullvm";
	if is_mingw_like {
		println!("cargo:rustc-link-arg=-Wl,--allow-multiple-definition");
	}

	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && is_mingw_like {
		println!("cargo:rustc-link-arg=-static");
	}

	let mut build = cxx_build::bridge("src/occt/ffi.rs");
	build.file("cpp/wrapper.cpp").include(occt_include).std("c++17").define("_USE_MATH_DEFINES", None);

	if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
		build.flag("/utf-8");
	}

	#[cfg(feature = "color")]
	build.define("CADRUM_COLOR", None);

	build.compile("cadrum_cpp");

	println!("cargo:rerun-if-changed=src/occt/ffi.rs");
	println!("cargo:rerun-if-changed=cpp/wrapper.h");
	println!("cargo:rerun-if-changed=cpp/wrapper.cpp");
}

/// Download a prebuilt OCCT tarball for `target` into `dest`.
#[cfg(not(feature = "source-build"))]
fn download_prebuilt(dest: &Path, target: &str) -> Option<[PathBuf; 2]> {
	let slug_ver = slug(OCCT_VERSION);
	let top_name = format!("cadrum-occt-{}-{}", slug_ver, target);
	let tarball_name = format!("{}.tar.gz", top_name);
	let url = env::var("CADRUM_PREBUILT_URL").unwrap_or_else(|_| format!("https://github.com/lzpel/cadrum/releases/download/{}/{}", OCCT_PREBUILT_TAG, tarball_name));

	eprintln!("cargo:warning=Downloading prebuilt OCCT from {}", url);

	let parent = dest.parent()?;
	std::fs::create_dir_all(parent).ok()?;

	if let Err(e) = download_and_extract_tar_gz(&url, parent) {
		eprintln!("cargo:warning=prebuilt fetch failed: {}", e);
		return None;
	}

	let extracted = parent.join(&top_name);
	if !extracted.is_dir() {
		eprintln!("cargo:warning=prebuilt tarball missing expected top-level dir `{}`", top_name);
		return None;
	}

	if extracted != *dest {
		let _ = std::fs::remove_dir_all(dest);
		if let Err(e) = std::fs::rename(&extracted, dest) {
			eprintln!("cargo:warning=failed to move extracted OCCT into {}: {}", dest.display(), e);
			return None;
		}
	}

	find_occt_dirs(dest)
}

fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<(), String> {
	let bytes = fetch_bytes(url)?;
	let gz = libflate::gzip::Decoder::new(&bytes[..]).map_err(|e| format!("gzip decode failed: {e}"))?;
	tar::Archive::new(gz).unpack(dest).map_err(|e| format!("tar unpack failed: {e}"))?;
	Ok(())
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
	if let Some(rest) = url.strip_prefix("file://") {
		let path: PathBuf = if rest.len() >= 3 && rest.starts_with('/') && rest.as_bytes()[2] == b':' {
			PathBuf::from(&rest[1..])
		} else {
			PathBuf::from(rest)
		};
		std::fs::read(&path).map_err(|e| format!("read {}: {}", path.display(), e))
	} else {
		let resp = minreq::get(url).send().map_err(|e| e.to_string())?;
		Ok(resp.into_bytes())
	}
}

// ---------------------------------------------------------------------------
// source-build: build OCCT from upstream sources.
// Dependencies on cmake and walkdir live here only.
// ---------------------------------------------------------------------------
#[cfg(feature = "source-build")]
mod source {
	use super::{download_and_extract_tar_gz, find_occt_dirs, OCCT_VERSION};
	use std::env;
	use std::path::{Path, PathBuf};

	/// Download OCCT source, patch, build with CMake, then remove non-patched
	/// source files (LGPL 2.1 §2: keep only the modified files).
	pub fn build_from_source(effective_root: &Path) -> Option<[PathBuf; 2]> {
		if let Some(dirs) = find_occt_dirs(effective_root) {
			return Some(dirs);
		}

		let occt_version = OCCT_VERSION;
		let occt_url = format!("https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/{}.tar.gz", occt_version);

		let download_dir = effective_root.join("occt-source");
		let extraction_sentinel = download_dir.join(".extraction_done");

		if !extraction_sentinel.exists() {
			std::fs::create_dir_all(&download_dir).unwrap();

			if let Ok(entries) = std::fs::read_dir(&download_dir) {
				for entry in entries.flatten() {
					let name = entry.file_name();
					if name.to_string_lossy().starts_with("OCCT") && entry.path().is_dir() {
						eprintln!("Removing partial OCCT extraction: {:?}", name);
						let _ = std::fs::remove_dir_all(entry.path());
					}
				}
			}

			eprintln!("Downloading OCCT {} from {} ...", occt_version, occt_url);
			download_and_extract_tar_gz(&occt_url, &download_dir).expect("Failed to download/extract OCCT source tarball");

			std::fs::write(&extraction_sentinel, "done").unwrap();
			eprintln!("OCCT source extracted successfully.");
		}

		let source_dir = std::fs::read_dir(&download_dir)
			.expect("Failed to read occt-source directory")
			.flatten()
			.find(|e| e.file_name().to_string_lossy().starts_with("OCCT") && e.path().is_dir())
			.map(|e| e.path())
			.expect("OCCT source directory not found after extraction");

		// Apply patches
		walk_occt_sources(&source_dir, |path| {
			if let Some(patched) = patch_or_none(path) {
				std::fs::write(path, patched).expect("patch write failed");
				eprintln!("Patched {}", path.file_name().unwrap().to_string_lossy());
			}
		});

		eprintln!("Building OCCT with CMake (this may take a while)...");

		let built = cmake::Config::new(&source_dir)
			.profile("Release")
			.define("BUILD_LIBRARY_TYPE", "Static")
			.define("CMAKE_INSTALL_PREFIX", effective_root.to_str().unwrap())
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
			.define("CMAKE_RC_FLAGS_INIT", "-C 1252")
			.build();

		eprintln!("OCCT built at: {}", built.display());

		// LGPL 2.1 §2: keep only patched files; remove everything else
		walk_occt_sources(&source_dir, |path| {
			if path.is_dir() {
				let _ = std::fs::remove_dir_all(path);
			} else if patch_or_none(path).is_none() {
				let _ = std::fs::remove_file(path);
			}
		});

		find_occt_dirs(effective_root)
	}

	/// Walk the OCCT source tree.
	/// - `src/` and `adm/`: recurse and yield every **file**
	/// - other top-level directories (`data/`, `dox/`, `tests/`, …): yield the **directory** itself
	/// - top-level files (`CMakeLists.txt`, `LICENSE_LGPL_21.txt`, …): skipped
	fn walk_occt_sources(source_dir: &Path, mut f: impl FnMut(&Path)) {
		for entry in walkdir::WalkDir::new(source_dir).min_depth(1).max_depth(1).into_iter().flatten() {
			let name = entry.file_name().to_string_lossy();
			if name == "src" || name == "adm" {
				for child in walkdir::WalkDir::new(entry.path()).into_iter().flatten() {
					if child.file_type().is_file() {
						f(child.path());
					}
				}
			} else if entry.file_type().is_dir() {
				f(entry.path());
			}
		}
	}

	/// Return the patched content for a file if it needs patching, `None` otherwise.
	/// Pure function — does not write to disk.
	fn patch_or_none(path: &Path) -> Option<String> {
		let name = path.file_name()?.to_str()?;
		let is_windows = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

		match name {
			"XCAFDoc_VisMaterial.cxx" => Some(stub_content(path, true)),
			"XCAFPrs_Texture.cxx" => Some(stub_content(path, false)),

			"Standard_StackTrace.cxx" => {
				let stubbed = stub_content(path, true);
				Some(comment_out_include_in(&stubbed, "execinfo.h"))
			}

			"OSD_WNT.cxx" if is_windows => Some(stub_content(path, false)),
			"OSD_File.cxx" | "OSD_Protection.cxx" | "OSD_signal.cxx"
			| "OSD_FileNode.cxx" | "OSD_Process.cxx"
				if is_windows =>
			{
				Some(stub_content(path, true))
			}

			"occt_defs_flags.cmake" if is_windows => {
				let content = std::fs::read_to_string(path).ok()?;
				let needle = "add_definitions(-DOCC_CONVERT_SIGNALS)";
				let replacement = "# add_definitions(-DOCC_CONVERT_SIGNALS)  # patched out by cadrum build.rs";
				if content.contains(needle) {
					Some(content.replace(needle, replacement))
				} else if content.contains(replacement) {
					Some(content) // already patched — keep as-is
				} else {
					None
				}
			}

			_ => None,
		}
	}

	/// Generate stubbed content for a C++ source file without writing to disk.
	fn stub_content(path: &Path, keep_signatures: bool) -> String {
		let unix = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_secs().to_string())
			.unwrap_or_else(|_| "unknown".to_string());
		let description = if keep_signatures { "method bodies stubbed" } else { "file emptied" };
		let header = format!("// Stubbed by cadrum build.rs at unix={unix}: {description}.\n");

		if keep_signatures {
			let content = std::fs::read_to_string(path).expect("Failed to read file for stubbing");
			header + &stub_all_top_level_bodies(&content)
		} else {
			header
		}
	}

	/// Comment out `#include <header>` in a string and return the result.
	fn comment_out_include_in(content: &str, header: &str) -> String {
		let needle = format!("#include <{}>", header);
		let replacement = format!("// {} (patched out by cadrum build.rs)", needle);
		content.replace(&needle, &replacement)
	}

	fn lex_normalize(content: &str) -> String {
		let bytes = content.as_bytes();
		let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
		let mut i = 0;
		let mut at_line_start = true;

		let push_blank = |out: &mut Vec<u8>, b: u8| {
			out.push(if b == b'\n' { b'\n' } else { b' ' });
		};

		while i < bytes.len() {
			let c = bytes[i];

			if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
				while i < bytes.len() && bytes[i] != b'\n' {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
				out.push(b' ');
				out.push(b' ');
				i += 2;
				while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
					push_blank(&mut out, bytes[i]);
					i += 1;
				}
				if i + 1 < bytes.len() {
					out.push(b' ');
					out.push(b' ');
					i += 2;
				} else {
					while i < bytes.len() {
						push_blank(&mut out, bytes[i]);
						i += 1;
					}
				}
				continue;
			}
			if c == b'"' {
				out.push(b' ');
				i += 1;
				while i < bytes.len() && bytes[i] != b'"' {
					if bytes[i] == b'\\' && i + 1 < bytes.len() {
						out.push(b' ');
						push_blank(&mut out, bytes[i + 1]);
						i += 2;
					} else {
						push_blank(&mut out, bytes[i]);
						i += 1;
					}
				}
				if i < bytes.len() {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if c == b'\'' {
				out.push(b' ');
				i += 1;
				while i < bytes.len() && bytes[i] != b'\'' {
					if bytes[i] == b'\\' && i + 1 < bytes.len() {
						out.push(b' ');
						out.push(b' ');
						i += 2;
					} else {
						out.push(b' ');
						i += 1;
					}
				}
				if i < bytes.len() {
					out.push(b' ');
					i += 1;
				}
				continue;
			}
			if at_line_start && c == b'#' {
				while i < bytes.len() {
					if bytes[i] == b'\n' {
						let mut k = i;
						while k > 0 && (bytes[k - 1] == b' ' || bytes[k - 1] == b'\t') {
							k -= 1;
						}
						let continued = k > 0 && bytes[k - 1] == b'\\';
						out.push(b'\n');
						i += 1;
						if !continued {
							break;
						}
					} else {
						out.push(b' ');
						i += 1;
					}
				}
				at_line_start = true;
				continue;
			}

			if c == b'\n' {
				at_line_start = true;
			} else if !c.is_ascii_whitespace() {
				at_line_start = false;
			}
			out.push(c);
			i += 1;
		}

		debug_assert_eq!(out.len(), bytes.len(), "lex_normalize must preserve byte length");
		String::from_utf8(out).expect("lex_normalize produced invalid utf-8")
	}

	fn stub_body_for_sig(sig: &str) -> &'static str {
		let sig_norm: String = {
			let mut s = sig.to_string();
			loop {
				let next = s.replace(" ::", "::").replace(":: ", "::");
				if next == s {
					break s;
				}
				s = next;
			}
		};

		let paren_pos = {
			let bytes = sig_norm.as_bytes();
			let mut cursor = 0;
			loop {
				let Some(off) = sig_norm[cursor..].find('(') else { return "{}"; };
				let pos = cursor + off;
				let before = sig_norm[..pos].trim_end();
				let id_start = before.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map(|p| p + 1).unwrap_or(0);
				let ident = &before[id_start..];
				let is_macro = !ident.is_empty()
					&& ident.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
					&& ident.chars().any(|c| c.is_ascii_uppercase());
				if !is_macro {
					break pos;
				}
				let mut depth = 1;
				let mut j = pos + 1;
				while j < bytes.len() && depth > 0 {
					match bytes[j] {
						b'(' => depth += 1,
						b')' => depth -= 1,
						_ => {}
					}
					j += 1;
				}
				cursor = j;
			}
		};
		let head_full = sig_norm[..paren_pos].trim();
		let head = head_full.rsplit('\n').next().unwrap_or(head_full).trim();
		if head.is_empty() {
			return "{}";
		}

		let hb = head.as_bytes();
		let mut start = hb.len();
		while start > 0 {
			let c = hb[start - 1];
			if c.is_ascii_alphanumeric() || c == b'_' || c == b':' || c == b'~' {
				start -= 1;
			} else {
				break;
			}
		}
		let name = &head[start..];
		let return_part = head[..start].trim();

		if name.contains('~') {
			return "{}";
		}
		let segs: Vec<&str> = name.split("::").collect();
		if segs.len() >= 2 && segs[segs.len() - 1] == segs[segs.len() - 2] {
			return "{}";
		}
		if return_part.is_empty() {
			return "{}";
		}

		let rb = return_part.as_bytes();
		let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
		let mut idx = 0;
		while let Some(off) = return_part[idx..].find("void") {
			let pos = idx + off;
			let end = pos + 4;
			let before_ok = pos == 0 || !is_ident(rb[pos - 1]);
			let after_ok = end >= rb.len() || !is_ident(rb[end]);
			if before_ok && after_ok {
				let mut j = end;
				while j < rb.len() && rb[j].is_ascii_whitespace() {
					j += 1;
				}
				if j >= rb.len() || (rb[j] != b'*' && rb[j] != b'&') {
					return "{}";
				}
			}
			idx = end;
		}

		"{ return {}; }"
	}

	fn stub_all_top_level_bodies(content: &str) -> String {
		let normalized = lex_normalize(content);
		let nb = normalized.as_bytes();
		let mut result = String::new();
		let mut depth = 0usize;
		let mut i = 0;
		let mut last_end = 0;

		while i < nb.len() {
			match nb[i] {
				b'{' if depth == 0 => {
					let brace_pos = i;
					let prefix_norm = &normalized[last_end..brace_pos];
					let sig = prefix_norm.rfind(|c| c == ';' || c == '}').map(|p| &prefix_norm[p + 1..]).unwrap_or(prefix_norm);

					let trimmed = sig.trim_end();
					let last_line = trimmed.rsplit('\n').next().unwrap_or(trimmed).trim();
					let is_function = {
						let mut t = last_line;
						loop {
							let prev_len = t.len();
							for kw in ["const", "override", "final", "noexcept", "mutable", "volatile", "= 0", "=0"] {
								if t.ends_with(kw) {
									t = t[..t.len() - kw.len()].trim_end();
									break;
								}
							}
							if t.len() == prev_len {
								break;
							}
						}
						t.ends_with(')')
					};
					let is_var_init = trimmed.ends_with('=') || !is_function;

					depth = 1;
					i += 1;
					while i < nb.len() && depth > 0 {
						match nb[i] {
							b'{' => depth += 1,
							b'}' => depth -= 1,
							_ => {}
						}
						i += 1;
					}

					if is_var_init {
						continue;
					}

					let stub_body = stub_body_for_sig(sig);
					result.push_str(&content[last_end..brace_pos]);
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
}
