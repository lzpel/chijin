//! Generate mdbook markdown and README examples section from numbered examples.
//! 番号付き example から mdbook 用 markdown と README の Examples 節を生成する。
//!
//! Usage / 使い方:
//!   cargo run --example markdown -- out/markdown/SUMMARY.md ./README.md
//!
//! 1. Discover NN_*.rs in examples/ / examples/ 配下の NN_*.rs を収集
//! 2. Run each example, collect outputs / 各 example を実行し生成物を回収
//! 3. Write SUMMARY.md + per-example .md / SUMMARY.md と各 example 用 .md を出力
//! 4. Update README.md ## Examples section / README.md の ## Examples 節を更新

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A numbered example file with its source content.
/// 番号付き example ファイルとそのソースコード。
struct Entry {
	path: PathBuf,    // absolute path to the .rs file / .rs ファイルの絶対パス
	content: String,  // source code / ソースコード
}

impl Entry {
	/// File stem, e.g. "01_primitives" / ファイル名（拡張子なし）
	fn stem(&self) -> &str {
		self.path.file_stem().unwrap().to_str().unwrap()
	}

	/// Display title, e.g. "Primitives" / 表示タイトル
	fn title(&self) -> String {
		let raw = self.stem()[3..].replace('_', " ");
		let mut chars = raw.chars();
		match chars.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
		}
	}

	/// First `//!` doc comment line as description / 冒頭の `//!` 行から説明文を抽出
	fn description(&self) -> &str {
		self.content.lines()
			.find(|l| l.starts_with("//!"))
			.map(|l| l.trim_start_matches("//!").trim())
			.unwrap_or("")
	}
}

/// Collect numbered example files (NN_*.rs) sorted by name.
/// 番号付き example (NN_*.rs) をファイル名順に収集する。
fn collect_entries() -> Vec<Entry> {
	let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
	let mut entries: Vec<Entry> = fs::read_dir(&examples_dir)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().into_string().ok()?;
			if name.len() >= 4 && name.ends_with(".rs") && name.as_bytes()[0].is_ascii_digit() && name.as_bytes()[1].is_ascii_digit() && name.as_bytes()[2] == b'_' {
				let path = e.path();
				let content = fs::read_to_string(&path).ok()?;
				Some(Entry { path, content })
			} else {
				None
			}
		})
		.collect();
	entries.sort_by(|a, b| a.stem().cmp(b.stem()));
	entries
}

/// Run each example in a temp directory and collect all generated files.
/// 一時ディレクトリで各 example を実行し、生成されたファイルを回収する。
fn collect_outputs(entries: &[Entry]) -> HashMap<PathBuf, Vec<u8>> {
	let tmp = std::env::temp_dir().join("cadrum_examples");
	clean_dir(&tmp);

	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
	for entry in entries {
		let stem = entry.stem();
		eprintln!("running example: {stem}");
		let status = Command::new("cargo")
			.args(["run", "--manifest-path", manifest.to_str().unwrap(), "--example", stem])
			.current_dir(&tmp)
			.status()
			.unwrap_or_else(|e| panic!("failed to run example {stem}: {e}"));
		assert!(status.success(), "example {stem} failed with {status}");
	}

	// Read all files from the temp directory / 一時ディレクトリの全ファイルを読み込む
	let outputs: HashMap<PathBuf, Vec<u8>> = fs::read_dir(&tmp)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let path = PathBuf::from(e.file_name());
			let contents = fs::read(e.path()).ok()?;
			Some((path, contents))
		})
		.collect();

	let _ = fs::remove_dir_all(&tmp);
	outputs
}

/// Return sorted asset PathBufs from outputs that belong to the given stem.
/// outputs から指定 stem に属するアセットのパスをソート済みで返す。
fn assets_for<'a>(outputs: &'a HashMap<PathBuf, Vec<u8>>, stem: &str) -> Vec<&'a PathBuf> {
	let mut names: Vec<&PathBuf> = outputs.keys()
		.filter(|p| {
			let name = p.to_str().unwrap_or("");
			name.starts_with(stem) && p.extension().map_or(false, |ext| matches!(ext.to_str(), Some("svg" | "png" | "step" | "brep")))
		})
		.collect();
	names.sort();
	names
}

/// Write output files, SUMMARY.md, and per-example markdown pages.
/// 生成物・SUMMARY.md・各 example の markdown ページを出力する。
fn write_summary(summary_path: &Path, entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) {
	let out_dir = summary_path.parent().expect("summary_path must have a parent directory");
	clean_dir(out_dir);

	// Write example output files (svg, step, brep, etc.) / example の生成物を書き出す
	for (path, contents) in outputs {
		fs::write(out_dir.join(path), contents).unwrap();
	}

	// Build SUMMARY.md and individual pages / SUMMARY.md と個別ページを生成する
	let mut summary = String::from("# Summary\n\n");
	for entry in entries {
		let (stem, title, desc) = (entry.stem(), entry.title(), entry.description());
		summary.push_str(&format!("- [{}]({}.md)\n", title, stem));

		// Format assets as markdown / 生成物を markdown 形式に変換
		let assets: String = assets_for(outputs, stem).iter()
			.map(|p| {
				let name = p.to_str().unwrap();
				match p.extension().and_then(|e| e.to_str()) {
					Some("svg" | "png") => format!("- {name}\n![img]({name})"),
					_ => format!("- [{name}]({name})"),
				}
			})
			.collect::<Vec<_>>()
			.join("\n\n");

		let desc_section = if desc.is_empty() { String::new() } else { format!("\n{}\n", desc) };
		let assets_section = if assets.is_empty() { String::new() } else { format!("\n{}", assets) };
		let md = format!("# {}\n{}\n```rust\n{}\n```{}", title, desc_section, entry.content, assets_section);
		fs::write(out_dir.join(format!("{}.md", stem)), md).unwrap();
	}

	fs::write(summary_path, &summary).unwrap();
	eprintln!("generated: {}", summary_path.display());
}

/// Update the ## Examples section in README.md, writing SVG assets to figure/examples/.
/// README.md の ## Examples 節を更新し、SVG アセットを figure/examples/ に書き出す。
fn write_readme(readme_path: &Path, entries: &[Entry], outputs: &HashMap<PathBuf, Vec<u8>>) {
	let readme = fs::read_to_string(readme_path).expect("failed to read README.md");

	// Find "## Examples" and the next "##" heading (or EOF) / ## Examples 節の範囲を特定
	let section_start = readme.find("\n## Examples").map(|i| i + 1)
		.expect("README.md must contain a ## Examples section");
	let section_end = readme[section_start + 1..].find("\n## ")
		.map(|i| section_start + 1 + i + 1)
		.unwrap_or(readme.len());

	// Write SVG/PNG assets to figure/examples/ / SVG/PNG を figure/examples/ に書き出す
	let figure_dir = readme_path.parent().unwrap().join("figure").join("examples");
	clean_dir(&figure_dir);
	for (path, contents) in outputs {
		if path.extension().map_or(false, |ext| matches!(ext.to_str(), Some("svg" | "png"))) {
			fs::write(figure_dir.join(path), contents).unwrap();
		}
	}

	// Build the new ## Examples section / 新しい ## Examples 節を構築
	let mut section = String::from("## Examples\n");
	for entry in entries {
		let (stem, title, desc) = (entry.stem(), entry.title(), entry.description());

		section.push_str(&format!("\n#### {}\n", title));
		if !desc.is_empty() {
			section.push_str(&format!("\n{}\n", desc));
		}
		section.push_str(&format!("\n```sh\ncargo run --example {}\n```\n", stem));
		section.push_str(&format!("\n```rust\n{}\n```\n", entry.content));

		// Embed first SVG/PNG as centered image / 最初の SVG/PNG を中央寄せで埋め込む
		let image = assets_for(outputs, stem).into_iter()
			.find(|p| p.extension().map_or(false, |ext| matches!(ext.to_str(), Some("svg" | "png"))));
		if let Some(img_path) = image {
			let img_name = img_path.to_str().unwrap();
			section.push_str(&format!(
				"\n<p align=\"center\">\n  <img src=\"figure/examples/{}\" alt=\"{}\" width=\"360\"/>\n</p>\n",
				img_name, stem
			));
		}
	}
	section.push('\n');

	// Replace the old section / 旧 Examples 節を差し替え
	let mut new_readme = String::with_capacity(readme.len());
	new_readme.push_str(&readme[..section_start]);
	new_readme.push_str(&section);
	new_readme.push_str(&readme[section_end..]);

	fs::write(readme_path, &new_readme).unwrap();
	eprintln!("updated: {}", readme_path.display());
}

/// Remove and recreate a directory.
/// ディレクトリを削除して再作成する。
fn clean_dir(dir: &Path) {
	if dir.exists() {
		fs::remove_dir_all(dir).expect("failed to clean directory");
	}
	fs::create_dir_all(dir).expect("failed to create directory");
}

fn main() {
	let entries = collect_entries();
	let outputs = collect_outputs(&entries);

	// Each arg is a file path: dispatch by filename / 各引数をファイル名で判別して処理
	for arg in std::env::args().skip(1) {
		let path = PathBuf::from(&arg);
		let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
		if name.starts_with("SUMMARY") {
			write_summary(&path, &entries, &outputs);
		} else if name.starts_with("README") {
			write_readme(&path, &entries, &outputs);
		} else {
			eprintln!("unknown target: {arg} (expected SUMMARY.md or README.md)");
		}
	}
}
