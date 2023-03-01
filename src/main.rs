use std::error::Error;
use std::path::Path;
use std::{env, fs, io, process};

use sha2::{Digest, Sha256};

use allsorts::binary::read::ReadScope;
use allsorts::fontfile::FontFile;
use allsorts::tables::svg::SvgTable;
use allsorts::tables::FontTableProvider;
use allsorts::tag;
use flate2::read::GzDecoder;
use std::io::Read;

const GZIP_HEADER: &[u8] = &[0x1F, 0x8B, 0x08];

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 2 {
        eprintln!("Usage: svg-dump path/to/SVGinOT.ttf [glyph id]");
        process::exit(2);
    }

    let res = if let Some(glyph_id_arg) = args.get(2) {
        dump_glyph(&args[1], glyph_id_arg)
    } else {
        hashes(&args[1])
    };

    match res {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Error: {}", err);
            process::exit(1);
        }
    }
}

enum GlyphToDump {
    Id(u16),
    All,
}

fn dump_glyph<P: AsRef<Path>>(path: P, glyph_id: &str) -> io::Result<()> {
    let glyph_id = match glyph_id {
        "all" => GlyphToDump::All,
        _ => GlyphToDump::Id(glyph_id.parse().map_err(to_io_error)?),
    };

    let buffer = fs::read(path)?;
    let scope = ReadScope::new(&buffer);
    let font_file = scope.read::<FontFile<'_>>().map_err(to_io_error)?;
    let table_provider = font_file.table_provider(0).map_err(to_io_error)?;
    let svg_data = table_provider
        .read_table_data(tag::SVG)
        .map_err(to_io_error)?;
    let svg = ReadScope::new(&svg_data).read::<SvgTable<'_>>().unwrap();

    for record in svg.document_records.iter_res() {
        let record = record.map_err(to_io_error)?;
        match glyph_id {
            GlyphToDump::All => {
                let svg_document = expand_document(record.svg_document)?;
                println!("{}", svg_document);
            }
            GlyphToDump::Id(id) if id >= record.start_glyph_id && id <= record.end_glyph_id => {
                let svg_document = expand_document(record.svg_document)?;
                println!("{}", svg_document);
                return Ok(());
            }
            _ => {}
        }
    }

    Ok(())
}

fn expand_document(data: &[u8]) -> io::Result<String> {
    let doc = if data.starts_with(GZIP_HEADER) {
        let mut gz = GzDecoder::new(data);
        let mut uncompressed = Vec::with_capacity(data.len());
        gz.read_to_end(&mut uncompressed)?;
        uncompressed
    } else {
        data.to_vec()
    };

    String::from_utf8(doc).map_err(to_io_error)
}

fn hashes<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let buffer = fs::read(path)?;
    let scope = ReadScope::new(&buffer);
    let font_file = scope.read::<FontFile<'_>>().map_err(to_io_error)?;
    let table_provider = font_file.table_provider(0).map_err(to_io_error)?;
    let svg_data = table_provider
        .read_table_data(tag::SVG)
        .map_err(to_io_error)?;
    let svg = ReadScope::new(&svg_data).read::<SvgTable<'_>>().unwrap();

    let mut hasher = Sha256::new();
    for record in svg.document_records.iter_res() {
        let record = record.map_err(to_io_error)?;
        hasher.update(record.svg_document);
        let hash = hasher.finalize_reset();
        println!(
            "{} → {}: {}",
            record.start_glyph_id,
            record.end_glyph_id,
            hexify(&hash)
        );
    }

    Ok(())
}

fn to_io_error<E: Into<Box<dyn Error + Send + Sync>>>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

fn hexify(bytes: &[u8]) -> String {
    use std::fmt::Write;

    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, byte| {
            write!(&mut s, "{:x}", byte).unwrap();
            s
        })
}
