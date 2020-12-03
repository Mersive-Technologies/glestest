#![cfg(target_os = "android")]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

extern crate android_logger;
extern crate regex;

use std::cmp::{max, min};
use std::fs::File;
use std::io::{Read, Write};
use std::time::Instant;

use anyhow::{anyhow, Context, Error};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use log::{debug, info, error};
use log::Level;

mod test_file;
use test_file::TestFile;

mod gl_convert;
use gl_convert::GlColorConverter;

#[no_mangle]
pub extern fn Java_com_mersive_glconvert_MainActivity_init(
    env: JNIEnv,
    _obj: JObject,
    path: JString,
) {
    android_logger::init_once(android_logger::Config::default().with_min_level(Level::Debug));
    info!("Hello, Rust!");

    let path: String = env.get_string(path).unwrap().into();

    // How many color conversion to do per test file.
    let num_runs = 100;

    // The compute group count is pretty arbitrary (with a max) and is based on the adreno chip.
    // It doesn't need to be a power of 2 or anything, we just need to make sure we correctly divy
    // work into them.
    // I think we can just optimize it for 1080p as long as lower resolutions can still go 60fps.
    // 69 (at mean 16.4ms) was the best number I got in 2-128 work groups, but it wasn't significantly
    // different from many other work group counts. Seems like as long as we're 8 or above, there
    // isn't a huge differences. Some higher numbers may be consistently lower, but not by much.

    let mut lowest_1080p_stat: Option<(usize, ProfStats)> = None;

    let min_groups = 32;
    let max_groups = 32;
    let save_output_file = true;

    for local_size in min_groups..=max_groups {
        let mut stats = vec![];
        let test_files: Vec<TestFile> = TestFile::get_test_files(&path).unwrap();
        for test_file in test_files {
            let stat = profile_color_conversion(&test_file, num_runs, local_size, save_output_file).unwrap();
            if test_file.height == 1080 {
                info!("1080p conversion with {} work groups: {} us", local_size, stat.mean_time_us);
                if lowest_1080p_stat.as_ref().is_none() || lowest_1080p_stat.as_ref().unwrap().1.mean_time_us > stat.mean_time_us {
                    // TODO: have some notion of significant difference
                    lowest_1080p_stat = Some((local_size, stat.clone()));
                    info!("New lowest 1080p conversion. Work groups: {}: {} us", local_size, stat.mean_time_us);
                }
            }
            stats.push(stat);
        }

        for stat in stats {
            info!("{:#?}", &stat);
        }
    }

    info!("Lowest 1080p work group num {}: {:#?}", lowest_1080p_stat.as_ref().unwrap().0, lowest_1080p_stat.as_ref().unwrap());
}

fn profile_color_conversion(test_file: &TestFile, num_runs: usize, local_size: usize, save_output: bool) -> Result<ProfStats, anyhow::Error> {
    let mut file = File::open(&test_file.path()).context("no file found")?;
    let metadata = std::fs::metadata(&test_file.path()).context("unable to read metadata")?;

    // load input image
    let mut data = vec![0; metadata.len() as usize];
    file.read(&mut data).context("buffer overflow")?;
    info!("Read {} byte image", data.len());

    let converter = GlColorConverter::new(test_file.width, test_file.height, local_size)
        .context("Failed to create color converter")?;

    let mut d = Vec::with_capacity(num_runs);
    for _i in 0..num_runs {
        let start = Instant::now();
        let out = converter.convert_frame(&data).context("Failed to convert frame")?;
        let duration = start.elapsed().as_micros();
        d.push(duration as u64);
    }
    let mean = d.iter().fold(0f64, |acc, &cur| acc + cur as f64) / num_runs as f64;
    let dev = (d.iter().fold(0f64, |acc, &cur| (cur as f64 - mean).powf(2f64) + acc) / num_runs as f64).sqrt();
    let sorted = d.sort();
    let median = d[num_runs / 2] as f64;
    let min = d.iter().fold(u64::max_value(), |acc, &cur| min(acc, cur)) as f64;
    let max = d.iter().fold(0u64, |acc, &cur| max(acc, cur)) as f64;

    let stats = ProfStats {
        test_file: test_file.clone(),
        mean_time_us: mean,
        median_time_us: median,
        std_dev_us: dev,
        min_time_us: min,
        max_time_us: max,
    };

    // Save
    if save_output {
        let out = converter.convert_frame(&data).context("Failed to convert frame")?;
        let path = format!("{}/converted_{}x{}.raw", &test_file.dir, test_file.width, test_file.height);
        info!("Writing file {}...", path);
        let mut file = File::create(path).context("Failed to create output file")?;
        file.write_all(&out).context("Failed to write output file")?;
    }

    Ok(stats)
}



#[derive(Debug,Clone)]
struct ProfStats {
    test_file: TestFile,
    mean_time_us: f64,
    median_time_us: f64,
    std_dev_us: f64,
    min_time_us: f64,
    max_time_us: f64,
}
