use std::cell::RefCell;
use std::error::Error;
use std::fmt::{Debug, Display};

use eframe::egui::{
    Color32 as EguiColor, ColorImage, Painter, Pos2, Rect as EguiRect, TextureOptions,
};
use smithay::backend::allocator::format::has_alpha;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::{
    sync::SyncPoint, DebugFlags, ImportDma, ImportDmaWl, ImportMem, ImportMemWl, Renderer,
    TextureFilter,
};
use smithay::backend::renderer::{Color32F, Frame, Texture};
use smithay::utils::{Buffer, Physical, Rectangle, Size, Transform};
use smithay::wayland::shm::{shm_format_to_fourcc, with_buffer_contents, BufferAccessError};

pub struct PolarBearRenderer {
    pub painter: Painter,
}

impl Debug for PolarBearRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolarBearRenderer").finish()
    }
}

impl ImportMemWl for PolarBearRenderer {
    fn import_shm_buffer(
        &mut self,
        buffer: &wayland_server::protocol::wl_buffer::WlBuffer,
        surface: Option<&smithay::wayland::compositor::SurfaceData>,
        damage: &[Rectangle<i32, Buffer>],
    ) -> Result<<Self as Renderer>::TextureId, <Self as Renderer>::Error> {
        with_buffer_contents(buffer, |ptr, len, data| {
            let offset = data.offset;
            let width = data.width;
            let height = data.height;
            let stride = data.stride;
            let fourcc = shm_format_to_fourcc(data.format);

            let color_image =
                ColorImage::from_rgba_unmultiplied([width as usize, height as usize], unsafe {
                    std::slice::from_raw_parts(ptr.add(offset as usize), len - offset as usize)
                });

            let texture =
                self.painter
                    .ctx()
                    .load_texture("", color_image, TextureOptions::default());
            Ok(PolarBearTexture(RefCell::new(texture)))
        })
        .map_err(|e| PolarBearRenderError(format!("{}", e)))?
    }
}

impl ImportMem for PolarBearRenderer {
    fn import_memory(
        &mut self,
        data: &[u8],
        format: Fourcc,
        size: Size<i32, Buffer>,
        _flipped: bool,
    ) -> Result<<Self as Renderer>::TextureId, <Self as Renderer>::Error> {
        assert_eq!(format, Fourcc::Argb8888);
        let color_image =
            ColorImage::from_rgba_unmultiplied([size.w as usize, size.h as usize], data);
        let texture = self
            .painter
            .ctx()
            .load_texture("", color_image, TextureOptions::default());
        Ok(PolarBearTexture(RefCell::new(texture)))
    }

    fn update_memory(
        &mut self,
        texture: &<Self as Renderer>::TextureId,
        data: &[u8],
        region: Rectangle<i32, Buffer>,
    ) -> Result<(), <Self as Renderer>::Error> {
        let region_x = region.loc.x as usize;
        let region_y = region.loc.y as usize;
        let region_width = region.size.w as usize;
        let region_height = region.size.h as usize;

        // Extract the data for the specified region
        // Assuming the full data represents the texture in row-major order
        let texture_width = region.size.w as usize; // Width of the full texture
        let mut subregion_data = Vec::with_capacity(region_width * region_height * 4);

        for row in 0..region_height {
            let start = (region_y + row) * texture_width * 4 + region_x * 4;
            let end = start + region_width * 4;
            subregion_data.extend_from_slice(&data[start..end]);
        }

        // Convert the extracted subregion into a ColorImage
        let color_image =
            ColorImage::from_rgba_unmultiplied([region_width, region_height], &subregion_data);

        // Define the position for the update
        let pos = [region_x, region_y];

        // Define any necessary texture options (or use default)
        let options = TextureOptions::default();

        // Update the specified region of the texture
        texture
            .0
            .borrow_mut()
            .set_partial(pos, color_image, options);

        Ok(())
    }

    fn mem_formats(&self) -> Box<dyn Iterator<Item = Fourcc>> {
        Box::new(vec![Fourcc::Argb8888, Fourcc::Xrgb8888].into_iter())
    }
}

impl ImportDmaWl for PolarBearRenderer {}

impl ImportDma for PolarBearRenderer {
    fn import_dmabuf(
        &mut self,
        _dmabuf: &smithay::backend::allocator::dmabuf::Dmabuf,
        _damage: Option<&[Rectangle<i32, Buffer>]>,
    ) -> Result<<Self as Renderer>::TextureId, <Self as Renderer>::Error> {
        Err(PolarBearRenderError("Dmabuf not yet supported".to_string()))
    }
}

pub struct SmithayRect(Rectangle<i32, Physical>);

impl Into<EguiRect> for SmithayRect {
    fn into(self) -> EguiRect {
        let min = Pos2 {
            x: self.0.loc.x as f32, // TO VERIFY: is this conversion appropriate?
            y: self.0.loc.y as f32,
        };
        let max = Pos2 {
            x: min.x + self.0.size.w as f32,
            y: min.y + self.0.size.h as f32,
        };
        EguiRect { min, max }
    }
}

pub struct SmithayColor(Color32F);

impl Into<EguiColor> for SmithayColor {
    fn into(self) -> EguiColor {
        EguiColor::from_rgba_unmultiplied(
            self.0.r() as u8,
            self.0.g() as u8,
            self.0.b() as u8,
            self.0.a() as u8,
        )
    }
}

#[derive(Debug)]
pub struct PolarBearRenderError(String);
impl Error for PolarBearRenderError {}
impl Display for PolarBearRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct PolarBearTexture(RefCell<eframe::egui::TextureHandle>);

impl Debug for PolarBearTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PolarBearTexture").finish()
    }
}

impl Texture for PolarBearTexture {
    fn width(&self) -> u32 {
        self.0.borrow().size()[0] as u32
    }

    fn height(&self) -> u32 {
        self.0.borrow().size()[1] as u32
    }

    fn format(&self) -> Option<Fourcc> {
        Some(Fourcc::Argb8888)
    }
}

pub struct PolarBearFrame<'a> {
    painter: &'a Painter,
}
impl Frame for PolarBearFrame<'_> {
    type Error = PolarBearRenderError;

    type TextureId = PolarBearTexture;

    fn id(&self) -> usize {
        usize::MAX
    }

    /// Clear the complete current target with a single given color.
    ///
    /// The `at` parameter specifies a set of rectangles to clear in the current target. This allows partially
    /// clearing the target which may be useful for damaged rendering.
    ///
    /// This operation is only valid in between a `begin` and `finish`-call.
    /// If called outside this operation may error-out, do nothing or modify future rendering results in any way.
    fn clear(
        &mut self,
        color: Color32F,
        at: &[Rectangle<i32, Physical>],
    ) -> Result<(), Self::Error> {
        for rect in at {
            self.painter.rect(
                SmithayRect(*rect).into(),
                0.0,
                SmithayColor(color),
                (0.0, EguiColor::TRANSPARENT),
            );
        }
        Ok(())
    }

    /// Draw a solid color to the current target at the specified destination with the specified color.
    fn draw_solid(
        &mut self,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        color: Color32F,
    ) -> Result<(), Self::Error> {
        self.painter.rect(
            SmithayRect(dst).into(),
            0.0,
            SmithayColor(color),
            (0.0, EguiColor::TRANSPARENT),
        );
        Ok(())
    }
    fn render_texture_from_to(
        &mut self,
        texture: &Self::TextureId,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        src_transform: Transform,
        alpha: f32,
    ) -> Result<(), Self::Error> {
        // Get texture size for UV normalization
        let texture_size = self.painter.ctx().available_rect();
        let texture_width = texture_size.width();
        let texture_height = texture_size.height();

        // Convert source rectangle to normalized UV coordinates
        let mut src_min = Pos2 {
            x: (src.loc.x as f32) / texture_width,
            y: (src.loc.y as f32) / texture_height,
        };
        let mut src_max = Pos2 {
            x: (src_min.x + src.size.w as f32) / texture_width,
            y: (src_min.y + src.size.h as f32) / texture_height,
        };

        // Apply transformations to UV coordinates
        match src_transform {
            Transform::Normal => {} // No-op
            Transform::Flipped180 => {
                src_min.x = 1.0 - src_min.x;
                src_min.y = 1.0 - src_min.y;
                src_max.x = 1.0 - src_max.x;
                src_max.y = 1.0 - src_max.y;
            }
            Transform::Flipped => {
                src_min.y = 1.0 - src_min.y;
                src_max.y = 1.0 - src_max.y;
            }
            _ => unimplemented!("Other transforms are not implemented yet"),
        }

        // Convert destination rectangle to screen space
        let dst_min = Pos2 {
            x: dst.loc.x as f32,
            y: dst.loc.y as f32,
        };
        let dst_max = Pos2 {
            x: dst_min.x + dst.size.w as f32,
            y: dst_min.y + dst.size.h as f32,
        };

        // Prepare rendering rectangles
        let rect = EguiRect {
            min: dst_min,
            max: dst_max,
        };
        let uv = EguiRect {
            min: src_min,
            max: src_max,
        };

        // Convert alpha to tint color
        let tint = EguiColor::from_white_alpha((alpha * 255.0) as u8);

        // Apply damage regions (if any)
        // for region in damage {
        //     self.painter.clip_rect(EguiRect {
        //         min: Pos2 {
        //             x: region.loc.x as f32,
        //             y: region.loc.y as f32,
        //         },
        //         max: Pos2 {
        //             x: (region.loc.x + region.size.w) as f32,
        //             y: (region.loc.y + region.size.h) as f32,
        //         },
        //     });
        // }

        // Perform rendering
        self.painter.image(texture.0.borrow().id(), rect, uv, tint);

        Ok(())
    }

    /// Output transformation that is applied to this frame
    fn transformation(&self) -> Transform {
        Transform::Normal
    }

    /// Wait for a [`SyncPoint`](sync::SyncPoint) to be signaled
    fn wait(
        &mut self,
        sync: &smithay::backend::renderer::sync::SyncPoint,
    ) -> Result<(), Self::Error> {
        sync.wait().map_err(|e| PolarBearRenderError(e.to_string()))
    }

    /// Finish this [`Frame`] returning any error that may happen during any cleanup.
    ///
    /// Dropping the frame instead may result in any of the following and is implementation dependent:
    /// - All actions done to the frame vanish and are never executed
    /// - A partial renderer with undefined framebuffer contents occurs
    /// - All actions are performed as normal without errors being returned.
    ///
    /// Leaking the frame instead will leak resources and can cause any of the previous effects.
    /// Leaking might make the renderer return Errors and force it's recreation.
    /// Leaking may not cause otherwise undefined behavior and program execution will always continue normally.
    fn finish(self) -> Result<smithay::backend::renderer::sync::SyncPoint, Self::Error> {
        // Perform any necessary cleanup or finalization of the rendering frame.

        // Generate a new SyncPoint, which might involve GPU-specific calls or Smithay utilities.
        let sync_point = smithay::backend::renderer::sync::SyncPoint::signaled();

        // Return the new SyncPoint to signal that rendering is complete.
        Ok(sync_point)
    }
}

impl Renderer for PolarBearRenderer {
    type Error = PolarBearRenderError;

    type TextureId = PolarBearTexture;

    type Frame<'frame> = PolarBearFrame<'frame>;

    /// Returns an id, that is unique to all renderers, that can use
    /// `TextureId`s originating from any of these renderers.
    fn id(&self) -> usize {
        usize::MAX
    }

    /// Set the filter method to be used when rendering a texture into a smaller area than its size
    fn downscale_filter(&mut self, filter: TextureFilter) -> Result<(), Self::Error> {
        Err(PolarBearRenderError(
            "Downscale filter not yet supported".to_string(),
        ))
    }

    /// Set the filter method to be used when rendering a texture into a larger area than its size
    fn upscale_filter(&mut self, _filter: TextureFilter) -> Result<(), Self::Error> {
        Err(PolarBearRenderError(
            "Upscale filter not yet supported".to_string(),
        ))
    }

    /// Set the enabled [`DebugFlags`]
    fn set_debug_flags(&mut self, _flags: DebugFlags) {}

    /// Returns the current enabled [`DebugFlags`]
    fn debug_flags(&self) -> DebugFlags {
        DebugFlags::empty()
    }

    /// Wait for a [`SyncPoint`](sync::SyncPoint) to be signaled
    fn wait(&mut self, sync: &SyncPoint) -> Result<(), Self::Error> {
        sync.wait().map_err(|e| PolarBearRenderError(e.to_string()))
    }

    /// Initialize a rendering context on the current rendering target with given dimensions and transformation.
    ///
    /// The `output_size` specifies the dimensions of the display **before** the `dst_transform` is
    /// applied.
    ///
    /// This function *may* error, if:
    /// - The given dimensions are unsupported (too large) for this renderer
    /// - The given Transformation is not supported by the renderer (`Transform::Normal` is always supported).
    /// - This renderer implements `Bind`, no target was bound *and* has no default target.
    /// - (Renderers not implementing `Bind` always have a default target.)
    fn render(
        &mut self,
        output_size: Size<i32, Physical>,
        dst_transform: Transform,
    ) -> Result<Self::Frame<'_>, Self::Error> {
        Ok(PolarBearFrame {
            painter: &self.painter,
        })
    }
}
