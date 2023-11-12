use apng::{load_dynamic_image, Encoder, Frame, PNGImage, ParallelEncoder};

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
            "encorde_parallel:{:?}",
            SystemTime::now().duration_since(start_time)
        );
    }
    {
        let start_time = SystemTime::now();
        parallel_encoder();
        println!(
            "parallel encoder:{:?}",
            SystemTime::now().duration_since(start_time)
        );
    }
}

fn encode_all() {
    let files = [
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
    let path = Path::new(r"out2.png");
    let out = BufWriter::new(File::create(path).unwrap());

    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    apng::Encoder::encode_parallel(out, Some(frame), 6, None, move |sender| {
        let files = [
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

fn parallel_encoder() {
    let path = Path::new(r"out3.png");

    let frame = Frame {
        delay_num: Some(1),
        delay_den: Some(2),
        ..Default::default()
    };

    let files = [
        "../_rust_logo/rust_logo1.png",
        "../_rust_logo/rust_logo2.png",
        "../_rust_logo/rust_logo3.png",
        "../_rust_logo/rust_logo4.png",
        "../_rust_logo/rust_logo5.png",
        "../_rust_logo/rust_logo6.png",
    ];

    let mut png_image_iter = files.iter().map(|file| {
        let mut file = File::open(file).unwrap();
        let mut buffer = vec![];
        file.read_to_end(&mut buffer).unwrap();
        let img = image::load_from_memory(&buffer).unwrap();
        load_dynamic_image(img).unwrap()
    });

    let first_frame = png_image_iter.next().unwrap();

    let encoder =
        ParallelEncoder::new(path.to_path_buf(), first_frame, Some(frame), 6, None, None).unwrap();
    png_image_iter.for_each(|image| {
        encoder.send(image);
    });
    encoder.finalize();
}
