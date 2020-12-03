use std::ffi::OsStr;
use std::fs::DirEntry;
use std::path::Path;

use regex::Regex;
use log::info;
use anyhow::{anyhow, Error};

#[derive(Debug,Clone)]
pub struct TestFile {
    pub dir: String,
    pub name: String,
    pub extension: String,
    pub width: usize,
    pub height: usize,
}

impl TestFile {
    pub fn new(dir: String, name: String) -> Result<TestFile, anyhow::Error> {
        let extension = TestFile::get_extension(&name)?.to_string();
        let (width, height) = TestFile::get_res_from_file_path(&name)?;
        let tf = TestFile {
            dir,
            name,
            extension,
            width,
            height,
        };
        Ok(tf)
    }

    pub fn path(&self) -> String {
        format!("{}/{}", &self.dir, &self.name)
    }

    pub fn get_test_files(dir: &String) -> Result<Vec<TestFile>, anyhow::Error> {
        let mut test_files = vec![];
        let files = std::fs::read_dir(dir)?;
        for file in files {
            if file.is_ok() {
                let file_path = TestFile::dir_entry_to_str(&file.unwrap());
                if file_path.is_ok() {
                    let file_path = file_path.unwrap();
                    info!("Found file: {}", &file_path);
                    let test_file = TestFile::new(dir.clone(), file_path);
                    if test_file.is_ok() {
                        test_files.push(test_file.unwrap());
                    }
                }
            }
        }
        Ok(test_files)
    }

    fn get_extension(file_name: &String) -> Result<&str, anyhow::Error> {
        let ext = Path::new(file_name).extension().and_then(OsStr::to_str)
            .ok_or(anyhow!(format!("Failed to get extension from: {}", file_name)))?;
        Ok(ext)
    }

    // Expects format such as: 1920x1080.raw
    fn get_res_from_file_path(file_path: &String) -> Result<(usize, usize), anyhow::Error> {
        let re = Regex::new(r"^\d{2,}x\d{2,}").unwrap();
        let mat = re.find(file_path).ok_or(anyhow!("Failed to parse resolution from: {}", file_path))?;
        let res_strs: Vec<&str> = mat.as_str().split('x').collect();
        match res_strs.len() {
            2 => {
                let width = res_strs[0].parse::<usize>()?;
                let height = res_strs[1].parse::<usize>()?;
                Ok((width, height))
            }
            _ => Err(anyhow!(format!("Failed to get resolution from: {}", file_path)))
        }
    }

    fn dir_entry_to_str(dir_entry: &DirEntry) -> Result<String, anyhow::Error> {
        let entry_str = dir_entry
            .path()
            .file_name()
            .ok_or(anyhow!("Bad dir entry file name"))?
            .to_string_lossy()
            .into_owned();
        Ok(entry_str)
    }
}