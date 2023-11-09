use apng::{load_dynamic_image, Encoder, Frame, PNGImage};

use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::Path;
use std::time::SystemTime;

fn main() {
    {
        let start_time = SystemTime::now();
        encode_all();
        println!(
            "encode all:{:?}",
            SystemTime::now().duration_since(start_time)
        );
    }
    {
        let start_time = SystemTime::now();
        encorde_parallel();
        println!(
            "generate apnng:{:?}",
            SystemTime::now().duration_since(start_time)
        );
    }
}

fn encode_all() {
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

    let path = Path::new(r"out.png");
    let mut out = BufWriter::new(File::create(path).unwrap());

    let config = apng::create_config(&png_images, None).unwrap();
    let mut encoder = Encoder::new(&mut out, config).unwrap();
    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    match encoder.encode_all(png_images, Some(&frame)) {
        Ok(_n) => println!("success"),
        Err(err) => eprintln!("{}", err),
    }
}

fn encorde_parallel() {
    let mut file = File::open("../_rust_logo/rust_logo1.png").unwrap();
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).unwrap();
    let img = image::load_from_memory(&buffer).unwrap();
    let png_image = load_dynamic_image(img).unwrap();

    let path = Path::new(r"out2.png");
    let out = BufWriter::new(File::create(path).unwrap());

    let config = apng::create_config_with_num_frames(&png_image, 6, None).unwrap();
    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    apng::Encoder::encode_parallel(out, config, Some(frame), move |sender| {
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

        png_images.into_iter().for_each(|image| {
            sender.send((image, None)).unwrap();
        });
    })
    .unwrap();
}
