use std::fs;
use std::path::PathBuf;

struct Entry {
	name: String,
	path: PathBuf,
}

fn collect_entries(examples_dir: &PathBuf) -> Vec<Entry> {
	let mut entries = fs::read_dir(examples_dir)
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().into_string().ok()?;
			if name.len() >= 3 && name.ends_with(".rs") && name.chars().nth(0)?.is_ascii_digit() && name.chars().nth(1)?.is_ascii_digit() && name.chars().nth(2)? == '_' {
				Some(Entry { name, path: e.path() })
			} else {
				None
			}
		})
		.collect::<Vec<_>>();
	entries.sort_by(|a, b| a.name.cmp(&b.name));
	entries
}

/// Returns markdown entries for all svg/png/step files in the current directory
/// whose filename contains `stem`, sorted by filename.
/// - svg/png: `"- {filename}\n![img]({filename})"`
/// - step:    `"- [{filename}]({filename})"`
fn collect_assets(stem: &str) -> Vec<String> {
	let mut files: Vec<String> = fs::read_dir(".")
		.unwrap()
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let name = e.file_name().into_string().ok()?;
			let ext = std::path::Path::new(&name).extension()?.to_str()?;
			if name.contains(stem) && matches!(ext, "svg" | "png" | "step") {
				Some(name)
			} else {
				None
			}
		})
		.collect();
	files.sort();
	files
		.into_iter()
		.map(|name| {
			let ext = std::path::Path::new(&name).extension().unwrap().to_str().unwrap();
			match ext {
				"svg" | "png" => format!("- {name}\n![img]({name})"),
				_ => format!("- [{name}]({name})"),
			}
		})
		.collect()
}

pub fn main() {
	let out = PathBuf::from(".");
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let entries = collect_entries(&manifest_dir.join("examples"));

	let mut summary = String::from("# Summary\n\n");
	for entry in &entries {
		let stem = entry.name.strip_suffix(".rs").unwrap();
		let title_raw = stem[3..].replace('_', " ");
		let mut chars = title_raw.chars();
		let display_title = match chars.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
		};

		summary.push_str(&format!("- [{}]({}.md)\n", display_title, stem));

		let source_code = fs::read_to_string(&entry.path).unwrap();
		let assets = collect_assets(stem).join("\n\n");
		let assets_section = if assets.is_empty() { String::new() } else { format!("\n\n{}", assets) };

		let md_content = format!("# {}\n\n```rust\n{}\n```{}", display_title, source_code, assets_section);
		fs::write(out.join(format!("{}.md", stem)), md_content).unwrap();
	}

	fs::write(out.join("SUMMARY.md"), summary).unwrap();
	println!("Generated: {:?}", out.join("SUMMARY.md"));
}
