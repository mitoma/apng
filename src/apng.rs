use super::errors::{APNGError, APNGResult};
use byteorder::{BigEndian, WriteBytesExt};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use flate2::Crc;
use rayon_ordered_bridge::bounded_parralel_map;
use rayon_ordered_bridge::bounded_parralel_map_channel;
use std::fs::File;
use std::io::BufWriter;
use std::io::{self, Write};
use std::mem;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use std::thread::JoinHandle;

use crate::png::PNGImage;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    // number of frames
    pub num_frames: u32,
    // count of loop, 0 is infinite looping
    pub num_plays: u32,
    pub color: png::ColorType,
    pub depth: png::BitDepth,
    pub filter: png::FilterType,
}

impl Config {
    // Returns the bits per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        self.color.samples() * self.depth as usize
    }

    // Returns the number of bytes needed for one deinterlaced row
    pub fn raw_row_length(&self) -> usize {
        let bits = self.width as usize * self.color.samples() * self.depth as usize;
        let extra = bits % 8;
        bits / 8
            + match extra {
                0 => 0,
                _ => 1,
            }
            + 1 // filter method
    }
}

pub struct ParallelEncoder {
    source_tx: SyncSender<PNGImage>,
    handler: JoinHandle<()>,
}

impl ParallelEncoder {
    const DEFAULT_CHANNEL_BOUND: usize = 32;

    pub fn new(
        path: PathBuf,
        image: PNGImage,
        default_frame: Option<Frame>,
        num_frames: u32,
        plays: Option<u32>,
        channel_bound: Option<usize>,
    ) -> APNGResult<ParallelEncoder> {
        let (source_tx, source_rx) = sync_channel(0);

        let handler = std::thread::spawn(move || {
            let writer = BufWriter::new(File::create(&path).unwrap());

            let config = create_config_with_num_frames(&image, num_frames, plays).unwrap();
            let image_buffer = ImageBuffer::new(&config, &image).unwrap();
            let mut encoder = Encoder::new(writer, config.clone()).unwrap();
            encoder
                .write_first_frame(&image_buffer, default_frame.as_ref())
                .unwrap();

            let (result, _waiter) = bounded_parralel_map(
                channel_bound.unwrap_or(Self::DEFAULT_CHANNEL_BOUND),
                source_rx.into_iter(),
                move |image| ImageBuffer::new(&config, &image).unwrap(),
            );

            for buf in result.iter() {
                encoder
                    .write_rest_frame(&buf, default_frame.as_ref())
                    .unwrap();
            }
            encoder.finish_encode().unwrap();
        });
        Ok(ParallelEncoder { source_tx, handler })
    }

    pub fn send(&self, image: PNGImage) {
        self.source_tx.send(image).unwrap();
    }

    pub fn finalize(self) {
        drop(self.source_tx);
        self.handler.join().unwrap();
    }
}

#[derive(Debug, PartialEq)]
pub struct Encoder<W: io::Write> {
    config: Config,
    w: W,
    seq_num: u32,
}

impl<W: io::Write> Encoder<W> {
    pub fn new(writer: W, config: Config) -> APNGResult<Self> {
        let mut e = Encoder {
            config,
            w: writer,
            seq_num: 0,
        };
        e.write_png_header()?;
        e.write_ihdr()?;
        e.write_ac_tl()?;
        Ok(e)
    }

    pub fn encode_parallel<F>(
        writer: W,
        default_frame: Option<Frame>,
        num_frames: u32,
        plays: Option<u32>,
        image_callback: F,
    ) -> APNGResult<()>
    where
        F: Fn(SyncSender<(PNGImage, Option<Frame>)>),
        F: Send + 'static,
    {
        let (source_tx, source_rx) = sync_channel::<(PNGImage, Option<Frame>)>(0);
        let (convert_tx, convert_rx, _waiter) = bounded_parralel_map_channel(
            32,
            move |(png_image, frame, config): (PNGImage, Option<Frame>, Config)| {
                (
                    ImageBuffer::new(&config, &png_image).unwrap(),
                    frame,
                    config,
                )
            },
        );

        rayon::spawn(move || {
            let (first_image, frame) = source_rx.recv().unwrap();
            let config = create_config_with_num_frames(&first_image, num_frames, plays).unwrap();
            convert_tx
                .send((first_image, frame, config.clone()))
                .unwrap();
            for (image, frame) in source_rx.iter() {
                convert_tx.send((image, frame, config.clone())).unwrap();
            }
        });

        rayon::spawn(move || {
            image_callback(source_tx);
        });

        let (image, frame, config) = convert_rx.recv().unwrap();
        let mut encoder = Self::new(writer, config).unwrap();
        encoder
            .write_first_frame(&image, frame.as_ref().or(default_frame.as_ref()))
            .unwrap();

        for (image_buffer, frame, _config) in convert_rx.iter() {
            encoder
                .write_rest_frame(&image_buffer, frame.as_ref().or(default_frame.as_ref()))
                .unwrap();
        }
        encoder.finish_encode().unwrap();
        Ok(())
    }

    // all png images encode to apng
    pub fn encode_all(&mut self, images: Vec<PNGImage>, frame: Option<&Frame>) -> APNGResult<()> {
        for (i, v) in images.iter().enumerate() {
            let image_buffer = ImageBuffer::new(&self.config, v)?;
            if i == 0 {
                self.write_first_frame(&image_buffer, frame)?;
            } else {
                self.write_rest_frame(&image_buffer, frame)?;
            }
        }
        self.write_iend()?;
        Ok(())
    }

    // write each frame control
    pub fn write_frame(&mut self, image: &PNGImage, frame: Frame) -> APNGResult<()> {
        let image_buffer = ImageBuffer::new(&self.config, image)?;
        if self.seq_num == 0 {
            self.write_first_frame(&image_buffer, Some(&frame))
        } else {
            self.write_rest_frame(&image_buffer, Some(&frame))
        }
    }

    fn write_first_frame(
        &mut self,
        image_buffer: &ImageBuffer,
        frame: Option<&Frame>,
    ) -> APNGResult<()> {
        self.write_fc_tl(frame)?;
        self.write_idats(image_buffer)
    }

    fn write_rest_frame(
        &mut self,
        image_buffer: &ImageBuffer,
        frame: Option<&Frame>,
    ) -> APNGResult<()> {
        self.write_fc_tl(frame)?;
        self.write_fd_at(image_buffer)
    }

    // finish encode, write end chunk on the last line.
    pub fn finish_encode(&mut self) -> APNGResult<()> {
        let encoded_frames = self.seq_num + 1;
        if self.config.num_frames > encoded_frames {
            return Err(APNGError::WrongFrameNums(
                self.config.num_frames as usize,
                encoded_frames as usize,
            ));
        }

        self.write_iend()
    }

    fn write_png_header(&mut self) -> APNGResult<()> {
        self.w.write_all(b"\x89PNG\r\n\x1a\n")?;
        Ok(())
    }

    fn write_iend(&mut self) -> APNGResult<()> {
        self.write_chunk(&[], *b"IEND")
    }

    fn write_ihdr(&mut self) -> APNGResult<()> {
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(self.config.width)?;
        buf.write_u32::<BigEndian>(self.config.height)?;
        buf.write_all(&[self.config.depth as u8, self.config.color as u8, 0, 0, 0])?;
        self.write_chunk(&buf, *b"IHDR")
    }

    fn write_ac_tl(&mut self) -> APNGResult<()> {
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(self.config.num_frames)?;
        buf.write_u32::<BigEndian>(self.config.num_plays)?;
        self.write_chunk(&buf, *b"acTL")
    }

    fn write_fc_tl(&mut self, frame: Option<&Frame>) -> APNGResult<()> {
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(self.seq_num)?;
        buf.write_u32::<BigEndian>(frame.and_then(|f| f.width).unwrap_or(self.config.width))?;
        buf.write_u32::<BigEndian>(frame.and_then(|f| f.height).unwrap_or(self.config.height))?;
        buf.write_u32::<BigEndian>(frame.and_then(|f| f.offset_x).unwrap_or(0))?;
        buf.write_u32::<BigEndian>(frame.and_then(|f| f.offset_y).unwrap_or(0))?;
        buf.write_u16::<BigEndian>(frame.and_then(|f| f.delay_num).unwrap_or(1))?;
        buf.write_u16::<BigEndian>(frame.and_then(|f| f.delay_den).unwrap_or(3))?;

        let dis = frame
            .and_then(|f| f.dispose_op)
            .unwrap_or(DisposeOp::ApngDisposeOpNone) as u8;
        let ble = frame
            .and_then(|f| f.blend_op)
            .unwrap_or(BlendOp::ApngBlendOpSource) as u8;
        buf.write_all(&[dis, ble])?;

        self.write_chunk(&buf, *b"fcTL")?;
        self.seq_num += 1;

        Ok(())
    }

    fn write_fd_at(&mut self, data: &ImageBuffer) -> APNGResult<()> {
        let mut buf = vec![];
        buf.write_u32::<BigEndian>(self.seq_num)?;
        buf.write_all(&data.0)?;
        self.write_chunk(&buf, *b"fdAT")?;
        self.seq_num += 1;
        Ok(())
    }

    // Writes the image data.
    fn write_idats(&mut self, data: &ImageBuffer) -> APNGResult<()> {
        self.write_chunk(&data.0, *b"IDAT")
    }

    // write chunk data 4 field
    fn write_chunk(&mut self, c_data: &[u8], c_type: [u8; 4]) -> APNGResult<()> {
        // Header(Length and Type)
        self.w.write_u32::<BigEndian>(c_data.len() as u32)?;
        self.w.write_all(&c_type)?;
        // Data
        self.w.write_all(c_data)?;
        // Footer (CRC)
        let mut crc = Crc::new();
        crc.update(&c_type);
        crc.update(c_data);
        self.w.write_u32::<BigEndian>(crc.sum())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub offset_x: Option<u32>,
    pub offset_y: Option<u32>,
    pub delay_num: Option<u16>,        // numerator of frame delay
    pub delay_den: Option<u16>,        // denominator of framge delay
    pub dispose_op: Option<DisposeOp>, // specifies before rendering the next frame
    pub blend_op: Option<BlendOp>, // specifies whether to blend alpha blend or replace the output buffer
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisposeOp {
    ApngDisposeOpNone = 0,
    ApngDisposeOpBackground = 1,
    ApngDisposeOpPrevious = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendOp {
    ApngBlendOpSource = 0,
    ApngBlendOpOver = 1,
}

pub fn create_config(images: &[PNGImage], plays: Option<u32>) -> APNGResult<Config> {
    if images.is_empty() {
        return Err(APNGError::ImagesNotFound);
    }
    let default_image = images[0].clone();
    Ok(Config {
        width: default_image.width,
        height: default_image.height,
        num_frames: images.len() as u32,
        num_plays: plays.unwrap_or(0),
        color: default_image.color_type,
        depth: default_image.bit_depth,
        filter: png::FilterType::NoFilter, //default
    })
}

pub fn create_config_with_num_frames(
    image: &PNGImage,
    num_frames: u32,
    plays: Option<u32>,
) -> APNGResult<Config> {
    Ok(Config {
        width: image.width,
        height: image.height,
        num_frames,
        num_plays: plays.unwrap_or(0),
        color: image.color_type,
        depth: image.bit_depth,
        filter: png::FilterType::NoFilter, //default
    })
}

fn filter_path(a: u8, b: u8, c: u8) -> u8 {
    let ia = i16::from(a);
    let ib = i16::from(b);
    let ic = i16::from(c);

    let p = ia + ib - ic;

    let pa = (p - ia).abs();
    let pb = (p - ib).abs();
    let pc = (p - ic).abs();

    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

pub fn filter(method: png::FilterType, bpp: usize, previous: &[u8], current: &mut [u8]) {
    use png::FilterType::*;
    assert!(bpp > 0);
    let len = current.len();

    match method {
        NoFilter => (),
        Sub => {
            for i in (bpp..len).rev() {
                current[i] = current[i].wrapping_sub(current[i - bpp]);
            }
        }
        Up => {
            for i in 0..len {
                current[i] = current[i].wrapping_sub(previous[i]);
            }
        }
        Avg => {
            for i in (bpp..len).rev() {
                current[i] =
                    current[i].wrapping_sub(current[i - bpp].wrapping_add(previous[i]) / 2);
            }

            for i in 0..bpp {
                current[i] = current[i].wrapping_sub(previous[i] / 2);
            }
        }
        Paeth => {
            for i in (bpp..len).rev() {
                current[i] = current[i].wrapping_sub(filter_path(
                    current[i - bpp],
                    previous[i],
                    previous[i - bpp],
                ));
            }

            for i in 0..bpp {
                current[i] = current[i].wrapping_sub(filter_path(0, previous[i], 0));
            }
        }
    }
}

struct ImageBuffer(Vec<u8>);

impl ImageBuffer {
    fn new(config: &Config, png_image: &PNGImage) -> APNGResult<ImageBuffer> {
        let data = &png_image.data;
        let mut buf = Vec::new();
        let bpp = config.bytes_per_pixel();
        let in_len = config.raw_row_length() - 1;

        let mut prev = vec![0; in_len];
        let mut current = vec![0; in_len];

        let data_size = in_len * config.height as usize;
        if data_size != data.len() {
            return Err(APNGError::WrongDataSize(data_size, data.len()));
        }

        let mut zlib = ZlibEncoder::new(&mut buf, Compression::best());
        let filter_method = config.filter;

        for line in data.chunks(in_len) {
            current.copy_from_slice(line);
            zlib.write_all(&[filter_method as u8])?;
            filter(filter_method, bpp, &prev, &mut current);
            zlib.write_all(&current)?;
            mem::swap(&mut prev, &mut current);
        }

        zlib.finish()?;
        Ok(ImageBuffer(buf))
    }
}
