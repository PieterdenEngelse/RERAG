use regex::Regex;
use std::fs;
use walkdir::WalkDir;

fn main() {
    let pattern = Regex::new(r"Arc\s*<\s*Mutex\s*<\s*Retriever\s*>>").unwrap();
    let root = "./src"; // adjust if your code lives elsewhere

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            for (i, line) in content.lines().enumerate() {
                if pattern.is_match(line) {
                    println!(
                        "Found in {} at line {}:\n    {}",
                        entry.path().display(),
                        i + 1,
                        line.trim()
                    );
                }
            }
        }
    }
}
