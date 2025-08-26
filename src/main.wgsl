@group(0) @binding(0)
var<storage, read_write> output: array<u32>;

@compute @workgroup_size(12)
fn main(
    @builtin(local_invocation_id) local_invocation_id: vec3u,
) {
    output[local_invocation_id[0]] = local_invocation_id[0];
}