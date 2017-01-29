extern crate rand;
extern crate winit;
extern crate glutin;

#[macro_use]
extern crate gfx;
extern crate gfx_window_glutin;

use rand::Rng;

use gfx::traits::FactoryExt;
use gfx::Device;

const QUAD_VERTICES: [Vertex; 4] = [Vertex { position: [-0.5, 0.5] },
                                    Vertex { position: [-0.5, -0.5] },
                                    Vertex { position: [0.5, -0.5] },
                                    Vertex { position: [0.5, 0.5] }];

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

pub type ColorFormat = gfx::format::Rgba8;
pub type DepthFormat = gfx::format::DepthStencil;

gfx_defines!{
    vertex Vertex {
        position: [f32; 2] = "a_Position",
    }

    // color format: 0xRRGGBBAA
    vertex Instance {
        translate: [f32; 2] = "a_Translate",
        color: u32 = "a_Color",
    }

    constant Locals {
        scale: f32 = "u_Scale",
    }

    pipeline pipe {
        vertex: gfx::VertexBuffer<Vertex> = (),
        instance: gfx::InstanceBuffer<Instance> = (),
        scale: gfx::Global<f32> = "u_Scale",
        locals: gfx::ConstantBuffer<Locals> = "Locals",
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

fn fill_instances(instances: &mut [Instance], instances_per_length: u32, size: f32) {
    let gap = 0.4 / (instances_per_length + 1) as f32;
    println!("gap: {}", gap);

    let begin = -1. + gap + (size / 2.);
    let mut translate = [begin, begin];
    let mut rng = rand::StdRng::new().unwrap();

    let length = instances_per_length as usize;
    for x in 0..length {
        for y in 0..length {
            let i = x * length + y;
            instances[i] = Instance {
                translate: translate,
                color: rng.next_u32(),
            };
            translate[1] += size + gap;
        }
        translate[1] = begin;
        translate[0] += size + gap;
    }
}

const MAX_INSTANCE_COUNT: usize = 2048;

struct App<R: gfx::Resources> {
    pso: gfx::PipelineState<R, pipe::Meta>,
    data: pipe::Data<R>,
    slice: gfx::Slice<R>,
    upload: gfx::handle::Buffer<R, Instance>,
    uploading: bool, // TODO: not needed if we have the encoder everywhere
}

impl<R> App<R>
    where R: gfx::Resources
{
    fn new<F>(factory: &mut F,
              color_format: gfx::handle::RenderTargetView<R,
                                                          (gfx::format::R8_G8_B8_A8,
                                                           gfx::format::Unorm)>)
              -> Self
        where F: gfx::Factory<R> + gfx::traits::FactoryExt<R>
    {

        let pso = factory.create_pipeline_simple(include_bytes!("shader/instancing_150.glslv"),
                                    include_bytes!("shader/instancing_150.glslf"),
                                    pipe::new())
            .unwrap();

        // let vs = gfx_app::shade::Source {
        //     glsl_120: include_bytes!("shader/instancing_120.glslv"),
        //     glsl_150: include_bytes!("shader/instancing_150.glslv"),
        //     msl_11: include_bytes!("shader/instancing_vertex.metal"),
        //     hlsl_40: include_bytes!("data/vertex.fx"),
        //     ..gfx_app::shade::Source::empty()
        // };
        // let fs = gfx_app::shade::Source {
        //     glsl_120: include_bytes!("shader/instancing_120.glslf"),
        //     glsl_150: include_bytes!("shader/instancing_150.glslf"),
        //     msl_11: include_bytes!("shader/instancing_frag.metal"),
        //     hlsl_40: include_bytes!("data/pixel.fx"),
        //     ..gfx_app::shade::Source::empty()
        // };

        let instances_per_length: u32 = 32;
        println!("{} instances per length", instances_per_length);
        let instance_count = instances_per_length * instances_per_length;
        println!("{} instances", instance_count);
        assert!(instance_count as usize <= MAX_INSTANCE_COUNT);
        let size = 1.6 / instances_per_length as f32;
        println!("size: {}", size);

        let upload = factory.create_upload_buffer(instance_count as usize).unwrap();
        {
            let mut writer = factory.write_mapping(&upload).unwrap();
            fill_instances(&mut writer, instances_per_length, size);
        }

        let instances = factory.create_buffer(instance_count as usize,
                           gfx::buffer::Role::Vertex,
                           gfx::memory::Usage::Data,
                           gfx::TRANSFER_DST)
            .unwrap();


        let (quad_vertices, mut slice) =
            factory.create_vertex_buffer_with_slice(&QUAD_VERTICES, &QUAD_INDICES[..]);
        slice.instances = Some((instance_count, 0));
        let locals = Locals { scale: size };

        App {
            pso: pso,
            data: pipe::Data {
                vertex: quad_vertices,
                instance: instances,
                scale: size,
                locals: factory.create_buffer_immutable(&[locals],
                                             gfx::buffer::Role::Constant,
                                             gfx::Bind::empty())
                    .unwrap(),
                out: color_format,
            },
            slice: slice,
            upload: upload,
            uploading: true,
        }
    }

    fn render<C: gfx::CommandBuffer<R>>(&mut self, encoder: &mut gfx::Encoder<R, C>) {
        if self.uploading {
            encoder.copy_buffer(&self.upload, &self.data.instance, 0, 0, self.upload.len())
                .unwrap();
            self.uploading = false;
        }

        encoder.clear(&self.data.out, [0.1, 0.2, 0.3, 1.0]);
        encoder.draw(&self.slice, &self.pso, &self.data);
    }
}

pub fn main() {
    let wb = glutin::WindowBuilder::new()
        .with_title("Triangle example".to_string())
        .with_dimensions(1024, 768)
        .with_vsync();

    let (window, mut device, mut factory, main_color, mut _main_depth) =
        gfx_window_glutin::init::<ColorFormat, DepthFormat>(wb);
    let mut app = App::new(&mut factory, main_color);

    let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();


    // loop over events

    'main: loop {
        for event in window.poll_events() {
            match event {
                glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) |
                glutin::Event::Closed => break 'main,
                glutin::Event::Resized(_width, _height) => {
                    // gfx_window_glutin::update_views(&window, &mut data.out, &mut main_depth);
                }
                _ => {}
            }
        }
        // draw a frame
        app.render(&mut encoder);
        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}
