//! The media thread, proven with the real driver: `anyview_media::Driver` is built and polled only on an
//! actor thread of the runtime, mpv's wake callback wakes that actor, each frame is announced to
//! this thread through a mailbox whose waker stands for `TextureHandle::redraw`, and this thread
//! (the UI thread) samples the player's texture in its own render pass and reads the pixels back.
//! The player's docs say "call `poll` on the thread that presents"; what the viewer relies on is
//! that polling off that thread and sampling on this one works (FINDINGS, "`Player::poll` works from
//! a thread that does not present").
//!
//! Ignored by default: it needs a wgpu adapter, an `mpv` and mpv-wgpu's C plugin (`MPV_WGPU_MPV`,
//! `MPV_WGPU_CPLUGIN`) (`cargo test -p anyview --test media_thread -- --ignored --nocapture`). The fixture is the media crate's own.
//!
//! Its own binary: a manual rig that needs a real wgpu adapter and mpv's C plugin and samples a texture across threads.

use anyview::runtime::{Actor, ActorBody, ActorWake, Flow, Mailbox, Outbox, UiWaker};
use anyview_core::{FilePath, MediaTime};
use anyview_media::{
    AudioDriver, Continuation, Driver, FrameSink, MediaCommand, MediaDriver, MediaEvent,
    PictureSlot,
};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn fixture(name: &str) -> FilePath {
    FilePath::new(
        std::fs::canonicalize(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../anyview-media/tests/fixtures")
                .join(name),
        )
        .expect("fixture"),
    )
    .expect("absolute")
}

/// The programs the player runs: `MPV_WGPU_MPV` and `MPV_WGPU_CPLUGIN` name them.
fn host() -> anyview_media::MpvHost {
    let path = |variable: &str| {
        PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| panic!("{variable} is not set")))
    };
    anyview_media::MpvHost {
        mpv: path("MPV_WGPU_MPV"),
        cplugin: path("MPV_WGPU_CPLUGIN"),
    }
}

/// The window's device, as far as a player is concerned.
fn open_device() -> (wgpu::Device, wgpu::Queue, String) {
    // One opening at a time in this process: the Vulkan loader crashes on two at once.
    let _one_at_a_time = anyview_media::GPU_OPENING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    for fallback in [false, true] {
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: fallback,
            }))
        else {
            continue;
        };
        let name = adapter.get_info().name;
        if let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("window-device"),
                ..Default::default()
            }))
        {
            return (device, queue, name);
        }
    }
    panic!("no wgpu adapter");
}

/// Stands for `TextureHandle::redraw`: the media thread asks the UI thread to paint.
struct Redraw(Sender<()>);

impl UiWaker for Redraw {
    fn wake(&self) {
        let _ = self.0.send(());
    }
}

enum MediaNote {
    Loaded,
    /// The player rewrote its texture on `polled_on`; `view` is what a layer would register.
    Frame {
        view: wgpu::TextureView,
        polled_on: ThreadId,
    },
}

/// What the driver's sink tells the UI thread: the texture and each frame, from the thread that
/// polled.
struct ToUi {
    outbox: Outbox<MediaNote>,
    last: Option<wgpu::TextureView>,
}

impl FrameSink for ToUi {
    fn texture(&mut self, view: &wgpu::TextureView) {
        self.last = Some(view.clone());
        self.frame();
    }

    fn frame(&mut self) {
        if let Some(view) = &self.last {
            self.outbox.send(MediaNote::Frame {
                view: view.clone(),
                polled_on: std::thread::current().id(),
            });
        }
    }

    fn cleared(&mut self) {
        self.last = None;
    }
}

struct Media {
    driver: Driver,
}

impl ActorBody for Media {
    type Command = MediaCommand;
    type Event = MediaNote;

    fn command(&mut self, command: MediaCommand, events: &Outbox<MediaNote>) -> Flow {
        let handled = self.driver.command(command);
        for event in handled.events {
            if matches!(event, MediaEvent::Loaded { .. }) {
                events.send(MediaNote::Loaded);
            }
        }
        match handled.then {
            Continuation::Keep => Flow::Continue,
            Continuation::Close => Flow::Quit,
        }
    }

    fn woken(&mut self, events: &Outbox<MediaNote>) {
        for event in self.driver.woken() {
            if matches!(event, MediaEvent::Loaded { .. }) {
                events.send(MediaNote::Loaded);
            }
        }
    }
}

impl Media {
    /// Runs on the actor thread: the driver (and the player in it) is built here, never on the UI
    /// thread.
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wake: ActorWake<MediaCommand>,
        events: Outbox<MediaNote>,
    ) -> Media {
        let driver = Driver::open(
            device,
            queue,
            AudioDriver::Null,
            &host(),
            &fixture("clip.mkv"),
            Box::new(ToUi {
                outbox: events,
                last: None,
            }),
            move || wake.wake(),
        )
        .expect("driver");
        let mut media = Media { driver };
        let slot = PictureSlot::Sized {
            width: NonZeroU32::new(WIDTH).expect("non-zero"),
            height: NonZeroU32::new(HEIGHT).expect("non-zero"),
        };
        let _ = media.driver.command(MediaCommand::Slot(slot));
        media
    }
}

/// The UI thread's render pass: samples a texture one-to-one into a target and reads it back.
struct Readback {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    target: wgpu::Texture,
    buffer: wgpu::Buffer,
}

impl Readback {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Readback {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sample"),
            source: wgpu::ShaderSource::Wgsl(
                "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
                    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
                }
                @group(0) @binding(0) var t: texture_2d<f32>;
                @fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
                    return textureLoad(t, vec2<i32>(pos.xy), 0);
                }"
                .into(),
            ),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sample"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(WIDTH * 4 * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Readback {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            layout,
            target,
            buffer,
        }
    }

    /// The pixels of `view` as this thread's own pass sees them.
    fn sample(&self, view: &wgpu::TextureView) -> Vec<u8> {
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        });
        let target_view = self.target.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (done, mapped) = channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = done.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll device");
        mapped.recv().expect("map callback").expect("map");
        let pixels = self.buffer.slice(..).get_mapped_range().to_vec();
        self.buffer.unmap();
        pixels
    }
}

struct Seen {
    frames: Vec<Vec<u8>>,
    polled_on: Vec<ThreadId>,
    loaded: bool,
}

/// Drains the mailbox on this thread until `done`, sampling every announced frame.
fn pump(
    mailbox: &Mailbox<MediaNote>,
    redraws: &std::sync::mpsc::Receiver<()>,
    readback: &Readback,
    seen: &mut Seen,
    done: impl Fn(&Seen) -> bool,
) {
    let started = Instant::now();
    while !done(seen) {
        assert!(
            started.elapsed() < TIMEOUT,
            "timed out; {} frames",
            seen.frames.len()
        );
        // Frames can only arrive through a redraw request from the media thread.
        let _ = redraws.recv_timeout(Duration::from_millis(500));
        for event in mailbox.drain() {
            match event {
                MediaNote::Loaded => seen.loaded = true,
                MediaNote::Frame { view, polled_on } => {
                    seen.polled_on.push(polled_on);
                    seen.frames.push(readback.sample(&view));
                }
            }
        }
    }
}

#[test]
#[ignore = "needs a wgpu adapter, mpv and its C plugin"]
fn a_media_thread_that_never_presents_polls_into_a_texture_another_thread_samples() {
    let (device, queue, adapter) = open_device();
    eprintln!("adapter: {adapter}");
    let ui_thread = std::thread::current().id();

    let (redraw_tx, redraws) = channel();
    let (mailbox, outbox) = Mailbox::new(Redraw(redraw_tx));
    let (media_device, media_queue) = (device.clone(), queue.clone());
    let for_sink = outbox.clone();
    let actor = Actor::spawn("anyview-media", outbox, move |wake| {
        Media::new(&media_device, &media_queue, wake, for_sink)
    })
    .expect("actor");

    let readback = Readback::new(&device, &queue);
    let mut seen = Seen {
        frames: Vec::new(),
        polled_on: Vec::new(),
        loaded: false,
    };
    pump(&mailbox, &redraws, &readback, &mut seen, |s| {
        s.loaded && s.frames.len() >= 8
    });

    // A command from this thread reaches the player on the media thread and playback goes on.
    let before = seen.frames.len();
    actor
        .send(MediaCommand::Seek(MediaTime::from_secs(2)))
        .expect("send seek");
    pump(&mailbox, &redraws, &readback, &mut seen, |s| {
        s.frames.len() >= before + 3
    });
    drop(actor);

    let polled: std::collections::HashSet<_> = seen.polled_on.iter().collect();
    assert_eq!(polled.len(), 1, "every poll ran on one thread");
    assert!(!polled.contains(&ui_thread), "no poll ran on the UI thread");

    let lit = |frame: &Vec<u8>| frame.chunks(4).any(|px| px[..3].iter().any(|c| *c != 0));
    assert!(seen.frames.iter().all(lit), "a sampled frame was black");
    assert!(
        seen.frames
            .iter()
            .all(|f| f.chunks(4).all(|px| px[3] == 255)),
        "alpha is 1 everywhere"
    );
    let mut distinct: Vec<&Vec<u8>> = Vec::new();
    for frame in &seen.frames {
        if !distinct.contains(&frame) {
            distinct.push(frame);
        }
    }
    assert!(
        distinct.len() >= 3,
        "{} distinct frames of {}",
        distinct.len(),
        seen.frames.len()
    );
    eprintln!(
        "{} frames sampled on the UI thread, {} distinct, all polled on {:?}",
        seen.frames.len(),
        distinct.len(),
        polled.iter().next()
    );
}
