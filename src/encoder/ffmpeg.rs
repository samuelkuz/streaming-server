use std::collections::VecDeque;
use std::collections::HashMap;
use ac_ffmpeg::codec::video::{PixelFormat, VideoEncoder, VideoFrame, VideoFrameMut};
use ac_ffmpeg::codec::{video, Encoder};
use ac_ffmpeg::time::{TimeBase, Timestamp};

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
        let time_base = TimeBase::new(1, 90_000);

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
            time_base: TimeBase::new(1, 90_000),
            width: width as usize,
            height: height as usize,
        }
    }

    pub fn encode(&mut self, frame_data: &[u8], frame_time: i64) -> Result<Vec<u8>> {
        let mut frame = self.take_frame();
        let time_base = frame.time_base();

        frame = frame
            .with_pts(Timestamp::new((frame_time as f64 * 9. / 1000.) as i64, time_base))
            .with_picture_type(video::frame::PictureType::None);
        
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
}
