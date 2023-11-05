use apng::{load_dynamic_image, Encoder, Frame, PNGImage};
use log::info;

use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::Path;
use std::time::SystemTime;

fn main() {
    std::env::set_var("RUST_LOG", "debug");
    //env_logger::builder().format_timestamp_millis().init();
    env_logger::builder().format_timestamp(None).init();

    {
        info!("start  serial");
        let start = SystemTime::now();
        gen_serial();
        info!(
            "finish serial. {:?}",
            SystemTime::now().duration_since(start)
        );
    }
    {
        info!("start  parallel");
        let start = SystemTime::now();
        gen_parallel();
        info!(
            "finish parallel. {:?}",
            SystemTime::now().duration_since(start)
        );
    }
}

fn gen_serial() {
    let files = vec![
        "../_rust_logo/rust_logo1.png",
        "../_rust_logo/rust_logo2.png",
        "../_rust_logo/rust_logo3.png",
        "../_rust_logo/rust_logo4.png",
        "../_rust_logo/rust_logo5.png",
        "../_rust_logo/rust_logo6.png",
    ];

    let mut png_images: Vec<PNGImage> = Vec::new();

    for f in files.iter() {
        let mut file = File::open(f).unwrap();
        let mut buffer = vec![];
        file.read_to_end(&mut buffer).unwrap();
        let img = image::load_from_memory(&buffer).unwrap();
        png_images.push(load_dynamic_image(img).unwrap());
    }

    let path = Path::new(r"out1.png");
    let mut out = BufWriter::new(File::create(path).unwrap());

    let config = apng::create_config(&png_images, None).unwrap();
    let mut encoder = Encoder::new(&mut out, config).unwrap();
    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    match encoder.encode_all(png_images, Some(&frame)) {
        Ok(_n) => {}
        Err(err) => eprintln!("{}", err),
    }
}

fn gen_parallel() {
    let files = vec![
        "../_rust_logo/rust_logo1.png",
        "../_rust_logo/rust_logo2.png",
        "../_rust_logo/rust_logo3.png",
        "../_rust_logo/rust_logo4.png",
        "../_rust_logo/rust_logo5.png",
        "../_rust_logo/rust_logo6.png",
    ];

    let mut png_images: Vec<PNGImage> = Vec::new();
    /* png file path
    for f in files.iter() {
        png_images.push(apng::load_png(f).unwrap());
    }
    */

    for f in files.iter() {
        let mut file = File::open(f).unwrap();
        let mut buffer = vec![];
        file.read_to_end(&mut buffer).unwrap();
        let img = image::load_from_memory(&buffer).unwrap();
        png_images.push(load_dynamic_image(img).unwrap());
    }

    let path = Path::new(r"out2.png");
    let mut out = BufWriter::new(File::create(path).unwrap());

    let config = apng::create_config(&png_images, None).unwrap();
    let mut encoder = Encoder::new(&mut out, config).unwrap();
    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    match encoder.encode_all_parallel(png_images, Some(&frame)) {
        Ok(_n) => {}
        Err(err) => eprintln!("{}", err),
    }
}
