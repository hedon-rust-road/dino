use std::{
    collections::BTreeSet,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use bundler::run_bundle;
use glob::{glob, GlobError};

use crate::BUILD_DIR;

// get all files with extensions in a directory
pub(crate) fn get_files_with_exts(dir: &str, exts: &[&str]) -> anyhow::Result<BTreeSet<PathBuf>> {
    // glob all ts files
    let mut files = BTreeSet::new();
    for ext in exts {
        let rule = format!("{}/**/*.{}", dir, ext);
        let paths = glob(&rule)?.collect::<Result<BTreeSet<PathBuf>, GlobError>>()?;
        files.extend(paths);
    }
    Ok(files)
}

pub(crate) fn calc_project_hash(dir: &str) -> anyhow::Result<String> {
    calc_hash_for_files(dir, &["ts", "fs", "json"], 16)
}

pub(crate) fn calc_hash_for_files(dir: &str, exts: &[&str], len: usize) -> anyhow::Result<String> {
    let files = get_files_with_exts(dir, exts)?;
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update_reader(File::open(file)?)?;
    }
    let mut ret = hasher.finalize().to_string();
    ret.truncate(len);
    Ok(ret)
}

// calculate the project hash, if the hash is different,
// rebuild the project, otherwise return the project path.
pub(crate) fn build_project(dir: &str) -> anyhow::Result<(String, bool)> {
    let hash = calc_project_hash(dir)?;
    fs::create_dir_all(BUILD_DIR)?;
    let filename = format!("{}/{}.mjs", BUILD_DIR, hash);
    let config = format!("{}/{}.yml", BUILD_DIR, hash);
    let dst = Path::new(&filename);
    if dst.exists() {
        return Ok((filename, true));
    }

    // bundle the project
    let content = run_bundle("main.ts", &Default::default())?;
    fs::write(dst, content)?;
    let mut dst = File::create(config)?;
    let mut src = File::open("config.yml")?;
    io::copy(&mut src, &mut dst)?;

    Ok((filename, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_files_with_exts_should_word() -> anyhow::Result<()> {
        let files = get_files_with_exts("fixtures/prj", &["ts", "js", "json"])?;
        assert_eq!(
            files.into_iter().collect::<Vec<_>>(),
            [
                PathBuf::from("fixtures/prj/a.ts"),
                PathBuf::from("fixtures/prj/test1/b.ts"),
                PathBuf::from("fixtures/prj/test1/c.js"),
                PathBuf::from("fixtures/prj/test2/test3/d.json"),
            ]
        );
        Ok(())
    }

    #[test]
    fn calc_hash_for_filesshould_work() -> anyhow::Result<()> {
        let hash = calc_hash_for_files("fixtures/prj", &["ts", "js", "json"], 12)?;
        assert_eq!(hash, "af1349b9f5f9");
        Ok(())
    }
}
