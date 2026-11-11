use wasm_bindgen::prelude::*;

/// Derive native ColoredPencil paper scale from explicit system-display pixels.
#[wasm_bindgen]
pub fn colored_pencil_depth_context(
    width: u32,
    height: u32,
    display_width: u32,
    display_height: u32,
) -> Result<JsValue, JsError> {
    let context = sdocx::InkRasterDepthContext::from_colored_pencil_display(
        width,
        height,
        display_width,
        display_height,
    )
    .map_err(js_error)?;
    serde_wasm_bindgen::to_value(&context).map_err(js_error)
}

/// Explicit pixel canvas; normal DocumentSession rendering remains independent.
#[wasm_bindgen]
pub struct InkRasterCanvas {
    canvas: sdocx::InkRasterCanvas,
}

#[wasm_bindgen]
impl InkRasterCanvas {
    /// The backdrop is top-origin premultiplied RGBA8 at the final resolution.
    #[wasm_bindgen(constructor)]
    pub fn new(context: JsValue, backdrop: js_sys::Uint8Array) -> Result<InkRasterCanvas, JsError> {
        let context: sdocx::InkRasterContext =
            serde_wasm_bindgen::from_value(context).map_err(js_error)?;
        let count = context.pixel_count().map_err(js_error)?;
        if backdrop.length() as usize != count * 4 {
            return Err(js_error(sdocx::InkRasterError::InvalidBackdrop));
        }
        let mut pixels = vec![[0; 4]; count];
        backdrop.copy_to(pixels.as_flattened_mut());
        let canvas = sdocx::InkRasterCanvas::from_pixels(context, pixels).map_err(js_error)?;
        Ok(Self { canvas })
    }

    pub fn paint(&mut self, stroke: JsValue, profile: JsValue) -> Result<(), JsError> {
        let stroke: sdocx::Stroke = serde_wasm_bindgen::from_value(stroke).map_err(js_error)?;
        let profile: sdocx::InkRasterProfile =
            serde_wasm_bindgen::from_value(profile).map_err(js_error)?;
        self.canvas.paint(&stroke, profile).map_err(js_error)
    }

    /// Paint Pencil2 with framebuffer-pixel translation: positive X is right
    /// and positive Y is up. The grain remains fixed to the framebuffer.
    pub fn paint_pencil2_translated(
        &mut self,
        stroke: JsValue,
        #[wasm_bindgen(unchecked_param_type = "'One' | 'Half'")] inverse_scale: JsValue,
        translation_x: f32,
        translation_y: f32,
    ) -> Result<(), JsError> {
        let stroke: sdocx::Stroke = serde_wasm_bindgen::from_value(stroke).map_err(js_error)?;
        let inverse_scale: sdocx::InkRasterInverseScale =
            serde_wasm_bindgen::from_value(inverse_scale).map_err(js_error)?;
        self.canvas
            .paint_pencil2_translated(&stroke, inverse_scale, [translation_x, translation_y])
            .map_err(js_error)
    }

    /// Paint one Pencil2 submission with an explicitly captured grain offset.
    /// Translation and offset are framebuffer pixels, X right and Y up; the
    /// offset is subtracted from pixel centers and does not move geometry.
    /// Supply the captured values without rounding; no offset is inferred.
    pub fn paint_pencil2_with_grain_offset(
        &mut self,
        stroke: JsValue,
        #[wasm_bindgen(unchecked_param_type = "'One' | 'Half'")] inverse_scale: JsValue,
        translation_x: f32,
        translation_y: f32,
        grain_offset_x: f32,
        grain_offset_y: f32,
    ) -> Result<(), JsError> {
        let stroke: sdocx::Stroke = serde_wasm_bindgen::from_value(stroke).map_err(js_error)?;
        let inverse_scale: sdocx::InkRasterInverseScale =
            serde_wasm_bindgen::from_value(inverse_scale).map_err(js_error)?;
        self.canvas
            .paint_pencil2_with_grain_offset(
                &stroke,
                inverse_scale,
                [translation_x, translation_y],
                [grain_offset_x, grain_offset_y],
            )
            .map_err(js_error)
    }

    /// Explicit native depth rows; only their red channel modulates coverage.
    pub fn paint_with_depth_map(
        &mut self,
        stroke: JsValue,
        profile: JsValue,
        depth: JsValue,
        pixels: js_sys::Uint8Array,
    ) -> Result<(), JsError> {
        let stroke: sdocx::Stroke = serde_wasm_bindgen::from_value(stroke).map_err(js_error)?;
        let profile: sdocx::InkRasterProfile =
            serde_wasm_bindgen::from_value(profile).map_err(js_error)?;
        let depth: sdocx::InkRasterDepthContext =
            serde_wasm_bindgen::from_value(depth).map_err(js_error)?;
        let count = depth.pixel_count().map_err(js_error)?;
        if pixels.length() as usize != count * 4 {
            return Err(js_error(sdocx::InkRasterError::InvalidDepthMap));
        }
        let bytes = pixels.to_vec();
        self.canvas
            .paint_with_depth_map(&stroke, profile, depth, bytes.as_chunks::<4>().0)
            .map_err(js_error)
    }

    pub fn pixels(&self) -> Vec<u8> {
        self.canvas.pixels().iter().flatten().copied().collect()
    }

    pub fn to_png(&self) -> Result<Vec<u8>, JsError> {
        self.canvas.to_png().map_err(js_error)
    }

    pub fn render_svg(&self) -> Result<String, JsError> {
        self.canvas
            .rendered_page(0)
            .map(|page| page.svg)
            .map_err(js_error)
    }

    /// Export the resolved pixel frame through the retained-scene PDF path.
    pub fn render_pdf(&self) -> Result<Vec<u8>, JsError> {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        self.canvas.to_pdf(&options).map_err(js_error)
    }
}

fn js_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}
