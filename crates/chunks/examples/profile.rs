//! Small reproducible encoder/compression benchmark. No network or authentication.
use smolmc_chunks::StoneChunk;
use smolmc_world::ChunkPos;
use std::{hint::black_box, io::Write, time::Instant};
fn main() {
    let start = Instant::now();
    let template = StoneChunk::new(42);
    let creation = start.elapsed();
    let mut scratch = Vec::with_capacity(template.shared_bytes() + 9);
    let start = Instant::now();
    for x in 0..100_000 {
        template.encode(black_box(ChunkPos { x, z: -x }), &mut scratch);
        black_box(&scratch);
    }
    let encode_ns = start.elapsed().as_nanos() / 100_000;
    let mut compressed_bytes = 0;
    let start = Instant::now();
    for x in 0..1000 {
        template.encode(ChunkPos { x, z: -x }, &mut scratch);
        let mut compressor =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        compressor.write_all(&scratch).unwrap();
        compressed_bytes += compressor.finish().unwrap().len();
    }
    let compression_ns = start.elapsed().as_nanos() / 1000;
    println!(
        "{{\"template_bytes\":{},\"packet_bytes\":{},\"scratch_capacity\":{},\"template_creation_ns\":{},\"encode_ns_per_chunk\":{},\"encode_iterations\":100000,\"encode_compress_ns_per_chunk\":{},\"compression_iterations\":1000,\"mean_zlib_bytes\":{}}}",
        template.shared_bytes(),
        scratch.len(),
        scratch.capacity(),
        creation.as_nanos(),
        encode_ns,
        compression_ns,
        compressed_bytes / 1000
    );
}
