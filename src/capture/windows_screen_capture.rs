use std::sync::mpsc::channel;
use std::slice;
use std::time::Instant;
use std::time::Duration;
use std::fs::File;
use std::io::Write;
use windows::core::{HSTRING, IInspectable, Interface};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Storage::{CreationCollisionOption, FileAccessMode, StorageFolder};
use windows::Graphics::DirectX::Direct3D11::IDirect3DSurface;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat};
use windows::Win32::Graphics::Direct3D11::{D3D11_BIND_FLAG, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ,
     D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D};
use crate::result::Result;
use crate::capture::d3d;
use crate::encoder::ffmpeg::{FfmpegEncoder};

pub struct WindowsScreenCapture<'a> {
    item: &'a GraphicsCaptureItem,
    device: ID3D11Device,
    d3d_context: ID3D11DeviceContext,
    frame_pool: Direct3D11CaptureFramePool,
}

impl<'a> WindowsScreenCapture<'a> {
    pub fn new(item: &'a GraphicsCaptureItem) -> Result<Self> {
        let item_size = item.Size()?;
        let (device, d3d_device, d3d_context) = d3d::create_direct3d_devices_and_context()?;
        let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item_size,
        )?;

        Ok(Self {
            item,
            device,
            d3d_context,
            frame_pool,
        })
    }

    unsafe fn surface_to_texture(&mut self, surface: &IDirect3DSurface) -> Result<ID3D11Texture2D> {
        let source_texture: ID3D11Texture2D = d3d::get_d3d_interface_from_object(surface)?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        source_texture.GetDesc(&mut desc);
        desc.BindFlags = D3D11_BIND_FLAG(0);
        desc.MiscFlags = D3D11_RESOURCE_MISC_FLAG(0);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        let copy_texture = self.device.CreateTexture2D(&desc, None)?;
        let src: ID3D11Resource = source_texture.cast()?;
        let dst: ID3D11Resource = copy_texture.cast()?;
        self.d3d_context.CopyResource(&dst, &src);
        Ok(copy_texture)
    }

    unsafe fn get_frame_content(
        &mut self,
        frame: Direct3D11CaptureFrame,
    ) -> Result<(ID3D11Resource, &[u8])> {
        let texture = self.surface_to_texture(&frame.Surface()?)?;
        let resource: ID3D11Resource = texture.cast()?;
        let mapped = self.d3d_context.Map(&resource, 0, D3D11_MAP_READ, 0)?;
        let frame: &[u8] = slice::from_raw_parts(
            mapped.pData as *const _,
            (self.item.Size()?.Height as u32 * mapped.RowPitch) as usize,
        );
        Ok((resource, frame))
    }

    pub async fn record(&mut self, mut encoder: FfmpegEncoder) -> Result<()> {
        let session = self.frame_pool.CreateCaptureSession(self.item)?;

        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Direct3D11CaptureFrame>(1);

        self.frame_pool.FrameArrived(
            &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
                move |frame_pool, _| {
                    let frame_pool = frame_pool.as_ref().unwrap();
                    let frame = frame_pool.TryGetNextFrame()?;
                    sender.try_send(frame).unwrap();
                    Ok(())
                }
            }),
        )?;

        session.StartCapture()?;

        let mut ticker =
            tokio::time::interval(Duration::from_millis((1000 / 30) as u64));
        
        let test_frames = 300;
        let mut count = 0;
        
        // create file
        let mut file = File::create("test.raw").unwrap();

        while let Some(frame) = receiver.recv().await {
            let frame_time = frame.SystemRelativeTime()?.Duration;
            let (resource, frame_bits) = unsafe { self.get_frame_content(frame)? };

            // encode here
            let encoded = encoder.encode(frame_bits, frame_time).unwrap();
            // file.write_all(&encoded)?;
            self.write(&mut file, &encoded).await.unwrap();

            unsafe {
                self.d3d_context.Unmap(&resource, 0);
            }

            if count == test_frames {
                break;
            }

            count += 1;

            ticker.tick().await;
        }

        session.Close()?;
        file.flush().unwrap();

        Ok(())
    }

    async fn write(&mut self, file: &mut File, input: &Vec<u8>) -> Result<()> {
        file.write_all(input)?;
        Ok(())
    }

    pub fn screenshot(&mut self) -> Result<()> {
        let session = self.frame_pool.CreateCaptureSession(self.item)?;

        let (sender, receiver) = channel();
        self.frame_pool.FrameArrived(
            &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new({
                move |frame_pool, _| {
                    let frame_pool = frame_pool.as_ref().unwrap();
                    let frame = frame_pool.TryGetNextFrame()?;
                    sender.send(frame).unwrap();
                    Ok(())
                }
            }),
        )?;

        session.StartCapture()?;

        let frame = receiver.recv().unwrap();

        let texture: ID3D11Texture2D = unsafe {
            session.Close()?;
            self.frame_pool.Close()?;
            self.surface_to_texture(&frame.Surface()?)
        }?;

        let bits = unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc as *mut _);

            let resource: ID3D11Resource = texture.cast()?;
            let mapped = self.d3d_context.Map(&resource, 0, D3D11_MAP_READ, 0)?;

            let frame: &[u8] = std::slice::from_raw_parts(
                mapped.pData as *const _,
                (desc.Height * mapped.RowPitch) as usize,
            );

            let bytes_per_pixel = 4;
            let mut bits = vec![0u8; (desc.Width * desc.Height * bytes_per_pixel) as usize];

            for row in 0..desc.Height {
                let data_begin = (row * (desc.Width * bytes_per_pixel)) as usize;
                let data_end = ((row + 1) * (desc.Width * bytes_per_pixel)) as usize;
                let slice_begin = (row * mapped.RowPitch) as usize;
                let slice_end = slice_begin + (desc.Width * bytes_per_pixel) as usize;
                bits[data_begin..data_end].copy_from_slice(&frame[slice_begin..slice_end]);
            }

            self.d3d_context.Unmap(&resource, 0);

            bits
        };

        let path = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let folder = StorageFolder::GetFolderFromPathAsync(&HSTRING::from(path.as_str()))?.get()?;
        let file = folder.
            CreateFileAsync(&HSTRING::from("screenshot.png"), CreationCollisionOption::ReplaceExisting)?
            .get()?;

        let item_size = self.item.Size()?;

        let stream = file.OpenAsync(FileAccessMode::ReadWrite)?.get()?;
        let encoder = BitmapEncoder::CreateAsync(BitmapEncoder::PngEncoderId()?, &stream)?.get()?;
        encoder.SetPixelData(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            item_size.Width as u32,
            item_size.Height as u32,
            1.0,
            1.0,
            &bits,
        )?;

        encoder.FlushAsync()?.get()?;

        Ok(())
    }
}


impl Drop for WindowsScreenCapture<'_> {
    fn drop(&mut self) {
        self.frame_pool.Close().unwrap();
    }
}
