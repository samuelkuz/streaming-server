use itertools::Itertools;
use std::num::Wrapping;
use std::collections::VecDeque;
use std::collections::HashMap;
use ac_ffmpeg::codec::video::{PixelFormat, VideoEncoder, VideoFrame, VideoFrameMut};
use ac_ffmpeg::codec::{video, Encoder};
use ac_ffmpeg::time::{TimeBase, Timestamp};
use std::time::Instant;

// use crate::encoder::frame_pool::FramePool;
use crate::result::Result;

pub struct FfmpegEncoder {
    encoder: VideoEncoder,
    frames: VecDeque<VideoFrame>,
    pixel_format: PixelFormat,
    time_base: TimeBase,
    width: usize,
    height: usize,
}

unsafe impl Send for FfmpegEncoder {}

impl FfmpegEncoder {
    pub fn new(width: u32, height: u32) -> Self {
        let time_base = TimeBase::new(1, 30);

        let pixel_format_option = "bgr0";
        let pixel_format = video::frame::get_pixel_format(pixel_format_option.clone());

        let encoder_option = "h264_nvenc";
        let mut encoder = VideoEncoder::builder(encoder_option.clone())
            .unwrap()
            .pixel_format(pixel_format)
            .width(width as _)
            .height(height as _)
            .time_base(time_base);

        let options: HashMap<String, String> = HashMap::from([
            ("profile".into(), "baseline".into()),
            ("preset".into(), "p7".into()),
            ("tune".into(), "ll".into()),
            ("zerolatency".into(), "true".into()),
            ("forced-idr".into(), "true".into()),
        ]);

        // let options: HashMap<String, String> = HashMap::from([
        //     ("profile".into(), "baseline".into()),
        //     ("preset".into(), "ultrafast".into()),
        //     ("tune".into(), "zerolatency".into()),
        // ]);

        for option in &options {
            encoder = encoder.set_option(option.0, option.1);
        }

        let encoder = encoder.build().unwrap();

        Self {
            encoder,
            frames: VecDeque::new(),
            pixel_format: pixel_format,
            time_base: TimeBase::new(1, 30),
            width: width as usize,
            height: height as usize,
        }
    }

    pub fn encode(&mut self, frame_data: &[u8], frame_idx: &i64) -> Result<Vec<u8>> {
        let mut frame = self.take_frame();
        let time_base = frame.time_base();

        // video::frame::PictureType::None
        // frame = frame
        //     .with_pts(Timestamp::new((frame_time as f64 * 9. / 1000.) as i64, time_base))
        //     .with_picture_type(video::frame::PictureType::None);

        let mut frame_timestamp = Timestamp::new(*frame_idx, time_base);
        // println!("ms: {}", frame_timestamp.as_millis().unwrap());
        
        frame = frame
            .with_pts(frame_timestamp)
            .with_picture_type(video::frame::PictureType::None);


        // yuv420p
        // self.convert_bgr_yuv(
        //     frame_data,
        //     frame
        //         .planes_mut()
        //         .iter_mut()
        //         .map(|p| p.data_mut())
        //         .collect(),
        // );
        
        // bgr
        frame.planes_mut()[0].data_mut().copy_from_slice(frame_data);

        let frame = frame.freeze();
        self.encoder.push(frame.clone())?;
        self.put_frame(frame);
        let mut ret = Vec::new();
        while let Some(packet) = self.encoder.take()? {
            ret.extend(packet.data());
        }

        Ok(ret)
    }

    fn put_frame(&mut self, frame: VideoFrame) {
        self.frames.push_back(frame);
    }

    fn take_frame(&mut self) -> VideoFrameMut {
        if let Some(frame) = self.frames.pop_front() {
            match frame.try_into_mut() {
                Ok(frame) => return frame,
                Err(frame) => self.frames.push_front(frame),
            }
        }

        VideoFrameMut::black(self.pixel_format, self.width as _, self.height as _)
            .with_time_base(self.time_base)
    }

    /// Converts the BGR8 array.
    #[allow(dead_code)]
    fn convert_bgr_yuv(&self, bgr: &[u8], yuv: Vec<&mut [u8]>) {
        let start = Instant::now();
        let mut upos = 0_usize;
        let mut vpos = 0_usize;
        let mut i = 0_usize;
        let (y, u, v) = yuv.into_iter().tuples().next().unwrap();

        for line in 0..self.height {
            if line % 2 != 0 {
                let mut x = 0_usize;
                while x < self.width {
                    let b = Wrapping(bgr[4 * i] as u32);
                    let g = Wrapping(bgr[4 * i + 1] as u32);
                    let r = Wrapping(bgr[4 * i + 2] as u32);

                    y[i] = (((Wrapping(66) * r + Wrapping(129) * g + Wrapping(25) * b) >> 8)
                        + Wrapping(16))
                    .0 as u8;
                    u[upos] = (((Wrapping(-38i8 as u32) * r
                        + Wrapping(-74i8 as u32) * g
                        + Wrapping(112) * b)
                        >> 8)
                        + Wrapping(128))
                    .0 as u8;
                    v[vpos] = (((Wrapping(112) * r
                        + Wrapping(-94i8 as u32) * g
                        + Wrapping(-18i8 as u32) * b)
                        >> 8)
                        + Wrapping(128))
                    .0 as u8;

                    i += 1;
                    upos += 1;
                    vpos += 1;

                    let b = Wrapping(bgr[4 * i] as u32);
                    let g = Wrapping(bgr[4 * i + 1] as u32);
                    let r = Wrapping(bgr[4 * i + 2] as u32);

                    y[i] = (((Wrapping(66) * r + Wrapping(129) * g + Wrapping(25) * b) >> 8)
                        + Wrapping(16))
                    .0 as u8;
                    i += 1;
                    x += 2;
                }
            } else {
                for _x in 0..self.width {
                    let b = Wrapping(bgr[4 * i] as u32);
                    let g = Wrapping(bgr[4 * i + 1] as u32);
                    let r = Wrapping(bgr[4 * i + 2] as u32);

                    y[i] = (((Wrapping(66) * r + Wrapping(129) * g + Wrapping(25) * b) >> 8)
                        + Wrapping(16))
                    .0 as u8;
                    i += 1;
                }
            }
        }

        let duration = start.elapsed();
        println!("Time elapsed in convert bgr8: {:?}", duration);
    }
}
