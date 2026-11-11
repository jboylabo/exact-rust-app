//! The macaron surface: a full-canvas fragment shader with one uniform
//! buffer, redrawn every frame from the frame clock.

use crate::shaders::macaron::{entry, module, Uniforms, GROUP_0, U};
use exact_gpu::wgpu;
use exact_gpu::{Frame, FrameUniform, Surface, SurfaceError, Value};

/// Seconds for one full turn about the vertical axis.
const TURN_SECONDS: f64 = 9.0;
/// Seconds for one bob up and down.
const BOB_SECONDS: f64 = 3.6;

/// The macaron surface. It takes no inputs.
#[derive(Default)]
pub struct MacaronSurface {
    gpu: Option<Gpu>,
}

struct Gpu {
    format: wgpu::TextureFormat,
    /// The shader generation the pipeline was built at.
    generation: u32,
    pipeline: wgpu::RenderPipeline,
    /// Written in place each frame; a bind group per slot.
    uniforms: FrameUniform,
    bind_groups: Vec<wgpu::BindGroup>,
}

impl MacaronSurface {
    /// A fresh surface.
    pub fn new() -> MacaronSurface {
        MacaronSurface::default()
    }
}

/// Where a cycle of `seconds` stands at `now_ms`, in radians. Wrapped in f64,
/// so the angle the shader gets stays precise however long the app has run.
pub fn phase(now_ms: f64, seconds: f64) -> f32 {
    let turns = (now_ms / 1000.0 / seconds).rem_euclid(1.0);
    (turns * std::f64::consts::TAU) as f32
}

impl Surface for MacaronSurface {
    fn bind(&mut self, inputs: &[Value], _: Option<f64>) -> Result<(), SurfaceError> {
        if !inputs.is_empty() {
            return Err(SurfaceError(format!(
                "macaron: expected no inputs, got {}",
                inputs.len()
            )));
        }
        Ok(())
    }

    fn render(
        &mut self,
        frame: &Frame,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> bool {
        if self.gpu.as_ref().map(|g| (g.format, g.generation))
            != Some((format, frame.shader_generation))
        {
            self.gpu = Some(build(device, format, frame.shader_generation));
        }
        let gpu = self.gpu.as_mut().unwrap();
        let (w, h) = frame.pixels();
        let uniforms = Uniforms {
            spin: phase(frame.now_ms, TURN_SECONDS),
            bob: phase(frame.now_ms, BOB_SECONDS),
            width: w as f32,
            height: h as f32,
        };
        let slot = gpu.uniforms.write(queue, &uniforms.bytes());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("macaron"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &gpu.bind_groups[slot], &[]);
            pass.draw(0..3, 0..1);
        }
        // It turns with the clock: another frame, always.
        true
    }
}

fn build(device: &wgpu::Device, format: wgpu::TextureFormat, generation: u32) -> Gpu {
    let shader = device.create_shader_module(module());
    let layout = device.create_bind_group_layout(&GROUP_0);
    let uniforms = FrameUniform::new(device, Uniforms::SIZE, "macaron uniforms");
    let bind_groups = (0..uniforms.slots())
        .map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("macaron"),
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: U.binding,
                    resource: uniforms.buffer(slot).as_entire_binding(),
                }],
            })
        })
        .collect();
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("macaron"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("macaron"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(entry::VS),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(entry::FS),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    Gpu {
        format,
        generation,
        pipeline,
        uniforms,
        bind_groups,
    }
}
