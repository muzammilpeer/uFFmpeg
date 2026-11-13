//! Developer-only provenance and license collection. Not installed with the editor.
use serde_json::{Value, json};
use std::{collections::BTreeSet, error::Error, fs, path::Path, process::Command};

fn copy_license_files(source: &Path, destination: &Path, depth: usize) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let upper = name.to_string_lossy().to_uppercase();
        if kind.is_symlink() {
            continue;
        }
        let in_license_directory = source
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("licenses"));
        let license_text = in_license_directory
            || ["LICENSE", "LICENCE", "COPYING"]
                .iter()
                .any(|p| upper.starts_with(p));
        if kind.is_file()
            && (license_text || ["NOTICE", "AUTHORS"].iter().any(|p| upper.starts_with(p)))
        {
            fs::create_dir_all(destination)?;
            fs::copy(entry.path(), destination.join(&name))?;
            count += usize::from(license_text);
        } else if kind.is_dir()
            && depth > 0
            && !matches!(upper.as_str(), ".GIT" | "TARGET" | "WORK" | "DIST")
        {
            count += copy_license_files(&entry.path(), &destination.join(&name), depth - 1)?;
        }
    }
    Ok(count)
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Usage: release-records OUTPUT_DIRECTORY RUST_TARGET".into());
    }
    let destination = Path::new(&args[0]);
    if destination.exists() {
        return Err("Records destination already exists; use a fresh directory".into());
    }
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            &args[1],
        ])
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata["packages"].as_array().ok_or("Missing packages")?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("Missing resolved dependencies")?;
    let mut reachable = metadata["workspace_members"]
        .as_array()
        .ok_or("Missing workspace members")?
        .iter()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect::<BTreeSet<_>>();
    loop {
        let before = reachable.len();
        for node in nodes {
            if node["id"].as_str().is_some_and(|id| reachable.contains(id))
                && let Some(deps) = node["dependencies"].as_array()
            {
                reachable.extend(deps.iter().filter_map(Value::as_str).map(String::from));
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    fs::create_dir_all(destination)?;
    let mut manifest = Vec::new();
    let mut notices = String::from(
        "uFFmpeg third-party dependency inventory\n\nThis is a collection of upstream declarations/texts, not legal approval. Review license alternatives and obligations before distribution. Slint's desktop royalty-free license must be reviewed explicitly; About Slint remains accessible in the app.\n\n",
    );
    let mut unresolved = Vec::new();
    for package in packages {
        let id = package["id"].as_str().ok_or("Missing package id")?;
        if !reachable.contains(id) {
            continue;
        }
        let name = package["name"].as_str().ok_or("Missing package name")?;
        let version = package["version"]
            .as_str()
            .ok_or("Missing package version")?;
        let key = format!("{name}-{version}");
        let root = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("Missing package manifest")?,
        )
        .parent()
        .ok_or("Missing package directory")?;
        let target = destination.join("licenses").join(&key);
        let mut count = copy_license_files(root, &target, 3)?;
        if let Some(file) = package["license_file"].as_str() {
            let candidate = root.join(file).canonicalize()?;
            // Workspace packages may inherit a root license; honor Cargo's explicit path.
            fs::create_dir_all(&target)?;
            fs::copy(candidate, target.join("DECLARED-LICENSE"))?;
            count += 1;
        }
        if count == 0 && package["source"].is_null() {
            let workspace = Path::new(
                metadata["workspace_root"]
                    .as_str()
                    .ok_or("Missing workspace root")?,
            );
            fs::create_dir_all(&target)?;
            fs::copy(workspace.join("LICENSE"), target.join("LICENSE"))?;
            count += 1;
        }
        if count == 0
            && let Some(inputs) = std::env::var_os("UFFMPEG_LICENSE_INPUTS")
        {
            let supplemental = Path::new(&inputs).join(&key);
            if supplemental.is_dir() {
                count += copy_license_files(&supplemental, &target, 3)?;
                fs::copy(
                    supplemental.join("PROVENANCE.txt"),
                    target.join("PROVENANCE.txt"),
                )?;
            }
        }
        if count == 0 {
            unresolved.push(key.clone());
        }
        let license = package["license"]
            .as_str()
            .unwrap_or("See declared license file");
        notices.push_str(&format!("{key}\nLicense expression: {license}\nRepository: {}\nLicense texts: licenses/{key}/\n\n", package["repository"].as_str().unwrap_or("not declared")));
        manifest.push(json!({"name":name,"version":version,"license":package["license"],"source":package["source"],"repository":package["repository"],"license_files":count}));
    }
    fs::write(destination.join("THIRD-PARTY.txt"), notices)?;
    fs::write(
        destination.join("dependencies.json"),
        serde_json::to_vec_pretty(&json!({"target":args[1],"packages":manifest}))?,
    )?;
    fs::write(
        destination.join("MISSING-LICENSE-TEXTS.txt"),
        unresolved.join("\n"),
    )?;
    if !unresolved.is_empty() {
        return Err("Some crates have no collected license text. Review MISSING-LICENSE-TEXTS.txt before packaging; no installer was produced.".into());
    }
    Ok(())
}
