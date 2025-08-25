@group(0) @binding(0)
var<storage, read_write> output: array<u32>;

@compute @workgroup_size(12)
fn main(
    @builtin(global_invocation_id) global_invocation_id: vec3u,
) {
    output[global_invocation_id[0]] = 1;
}