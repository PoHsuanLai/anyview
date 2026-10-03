//! The media architecture's open question: can `mpv-wgpu-player`'s `poll()` run on a dedicated
//! thread that never presents, writing into a texture made on the window's device, with the
//! redraw requested from that thread? The player's docs say "call `poll` on the thread that
//! presents".
//!
//! The rig is the viewer's own: a `Device` and `Queue` made on this thread stand for the window's
//! (headless, no window), the player is built and polled only on an actor thread of the runtime,
//! mpv's wake callback wakes that actor, and each rewritten frame is announced to this thread
//! through a mailbox whose waker stands for `TextureHandle::redraw`. This thread is the UI thread:
//! it samples the player's texture in its own render pass (what the compositor does with a
//! registered `TextureView`) and reads the pixels back.
//!
//! Ignored by default: it needs libmpv, a wgpu adapter and the fixtures of the sibling
//! `mpv-wgpu` checkout (`cargo test -p anyview --test media_thread -- --ignored --nocapture`).

use anyview::runtime::{Actor, ActorBody, ActorWake, Flow, Mailbox, Outbox, UiWaker};
use mpv_wgpu_player::{
    AudioOutput, Event, Finite, Player, PlayerOptions, Presentation, Seek, Slot, SlotSize,
};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../mpv/crates/mpv-wgpu-player/tests/fixtures")
        .join(name)
}

/// The window's device, as far as a player is concerned.
fn open_device() -> (wgpu::Device, wgpu::Queue, String) {
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

enum MediaCommand {
    SeekTo(f64),
}

enum MediaEvent {
    Loaded,
    /// The player rewrote its texture on `polled_on`; `view` is what a layer would register.
    Frame {
        view: wgpu::TextureView,
        polled_on: ThreadId,
    },
}

struct Media {
    player: Player,
}

impl Media {
    /// Runs on the actor thread: the player is built here, never on the UI thread.
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, wake: ActorWake<MediaCommand>) -> Media {
        let mut player = Player::new(
            device,
            queue,
            PlayerOptions {
                audio_output: AudioOutput::Null,
            },
        )
        .expect("player");
        player.set_notify(move || wake.wake());
        player
            .set_slot(Slot::Sized(SlotSize {
                width: NonZeroU32::new(WIDTH).expect("non-zero"),
                height: NonZeroU32::new(HEIGHT).expect("non-zero"),
            }))
            .expect("slot");
        let path = fixture("clip.mkv");
        player
            .load(path.to_str().expect("utf-8"))
            .expect("loadfile");
        Media { player }
    }
}

impl ActorBody for Media {
    type Command = MediaCommand;
    type Event = MediaEvent;

    fn command(&mut self, command: MediaCommand, _: &Outbox<MediaEvent>) -> Flow {
        match command {
            MediaCommand::SeekTo(seconds) => {
                let to = Finite::new(seconds).expect("finite");
                self.player.seek(Seek::Absolute(to)).expect("seek");
            }
        }
        Flow::Continue
    }

    fn woken(&mut self, events: &Outbox<MediaEvent>) {
        let outcome = self.player.poll().expect("poll");
        for event in self.player.events() {
            eprintln!("media event: {event:?}");
            if *event == Event::Loaded {
                events.send(MediaEvent::Loaded);
            }
        }
        match (outcome.presentation, self.player.picture()) {
            (Presentation::Updated, mpv_wgpu_player::Picture::Shown(view)) => {
                events.send(MediaEvent::Frame {
                    view: view.clone(),
                    polled_on: std::thread::current().id(),
                });
            }
            (Presentation::Updated, mpv_wgpu_player::Picture::Waiting)
            | (Presentation::Unchanged, _) => {}
        }
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
    mailbox: &Mailbox<MediaEvent>,
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
                MediaEvent::Loaded => seen.loaded = true,
                MediaEvent::Frame { view, polled_on } => {
                    seen.polled_on.push(polled_on);
                    seen.frames.push(readback.sample(&view));
                }
            }
        }
    }
}

#[test]
#[ignore = "needs libmpv, a wgpu adapter and the mpv-wgpu checkout's fixtures"]
fn a_media_thread_that_never_presents_polls_into_a_texture_another_thread_samples() {
    if !fixture("clip.mkv").exists() {
        panic!("fixture missing: {}", fixture("clip.mkv").display());
    }
    let (device, queue, adapter) = open_device();
    eprintln!("adapter: {adapter}");
    let ui_thread = std::thread::current().id();

    let (redraw_tx, redraws) = channel();
    let (mailbox, outbox) = Mailbox::new(Redraw(redraw_tx));
    let (media_device, media_queue) = (device.clone(), queue.clone());
    let actor = Actor::spawn("anyview-media", outbox, move |wake| {
        Media::new(&media_device, &media_queue, wake)
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
    actor.send(MediaCommand::SeekTo(2.0)).expect("send seek");
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
