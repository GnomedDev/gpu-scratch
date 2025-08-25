use std::{borrow::Cow, error::Error};

use wgpu::{BufferDescriptor, ComputePassDescriptor};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    real_main().await
}

#[derive(Debug, thiserror::Error)]
pub enum InitializeError {
    #[error("Unable to find GPU adapter!")]
    NoAdapter,
    #[error("Unable to find GPU device!")]
    NoDevice,
}

async fn initialize_gpu() -> Result<(wgpu::Device, wgpu::Queue), InitializeError> {
    static ADAPTER_OPTIONS: wgpu::RequestAdapterOptions = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    };

    static DEVICE_OPTIONS: wgpu::DeviceDescriptor = wgpu::DeviceDescriptor {
        label: Some("device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    };

    let gpu = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let Ok(adapter) = gpu.request_adapter(&ADAPTER_OPTIONS).await else {
        return Err(InitializeError::NoAdapter);
    };

    let Ok((device, queue)) = adapter.request_device(&DEVICE_OPTIONS).await else {
        return Err(InitializeError::NoDevice);
    };

    Ok((device, queue))
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {}

/// Runs the `src/main.wgsl` shader on the GPU, copying the output to `output`.
///
/// This function
/// 1. Creates an intermediate working buffer.
/// 2. Compiles the shader into a module.
/// 3. Creates a CommandEncoder.
/// 4. Creates a ComputePipeline that contains the shader module.
/// 5. Creates a BindGroup that contains the working buffer.
/// 6. Creates a ComputePass with the ComputePipeline and BindGroup
/// 7. Encodes this ComputePass into the CommandEncoder.
/// 8. Encodes a copy from the intermediate buffer into `output`
/// 9. Finishes the encode.
fn construct_compute_shader(device: &wgpu::Device, output: &wgpu::Buffer) -> wgpu::CommandBuffer {
    const SHADER_OPTIONS: wgpu::ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
        label: Some("shader-main"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("main.wgsl"))),
    };

    static ENCODER_OPTIONS: wgpu::CommandEncoderDescriptor = wgpu::CommandEncoderDescriptor {
        label: Some("encoder"),
    };

    static BIND_GROUP_LAYOUT_OPTIONS: wgpu::BindGroupLayoutDescriptor =
        wgpu::BindGroupLayoutDescriptor {
            label: Some("bind-group-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: None,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            }],
        };

    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("buffer-intermediate"),
        size: output.size(),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(SHADER_OPTIONS);
    let bind_group_layout = device.create_bind_group_layout(&BIND_GROUP_LAYOUT_OPTIONS);
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline-layout-descriptor"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline_options = wgpu::ComputePipelineDescriptor {
        label: Some("compile-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: None,
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    };

    let bind_group_options = wgpu::BindGroupDescriptor {
        label: Some("bind-group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    };

    let mut encoder = device.create_command_encoder(&ENCODER_OPTIONS);
    {
        let compute_pipeline = device.create_compute_pipeline(&compute_pipeline_options);
        let bind_group = device.create_bind_group(&bind_group_options);

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor::default());
        pass.set_pipeline(&compute_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 0, 0);
    }

    encoder.copy_buffer_to_buffer(&buffer, 0, output, 0, output.size());
    encoder.finish()
}

async fn real_main() -> Result<(), Box<dyn Error>> {
    let (device, queue) = initialize_gpu().await?;

    let output = device.create_buffer(&BufferDescriptor {
        label: Some("output-buffer"),
        size: (12 * size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let index = queue.submit(std::iter::once(construct_compute_shader(&device, &output)));

    device.poll(wgpu::PollType::WaitForSubmissionIndex(index))?;
    println!("GPU Completed");

    output.map_async(wgpu::MapMode::Read, .., {
        let output = output.clone();
        move |result| {
            if let Err(err) = result {
                eprintln!("{err}");
                return;
            }

            println!("{:?}", &output.get_mapped_range(..)[..]);
        }
    });

    device.poll(wgpu::PollType::Wait)?;
    Ok(())
}
