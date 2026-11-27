//! Draws a sheet of creatures from random genomes, for reviewing a type by eye.
//!
//! ```text
//! cargo run -p avatar-vectors --example random_sheet -- \
//!     --type creature.sprouts [--version 1] [--seed 1001] [--count 16] \
//!     [--columns 4] [--cell 256] [--mood neutral | --moods] --out sheet.png
//! ```
//!
//! Each cell is [`avatar::render_image`] at `--cell` pixels, premultiplied
//! with the ground shadow kept, composited over white. Cells are framed by
//! 1 px light-grey lines and carry their index at the top left, drawn in
//! pixel digits so no font is needed. `--moods` draws 4 genomes in each of
//! the 7 moods instead, one row per genome with the moods in the order
//! neutral, working, happy, waiting, blocked, sad, sleepy; it ignores
//! `--count`, `--columns` and `--mood`.
//!
//! Genome `g` of a sheet is read from the BLAKE3 extendable output of the
//! text `avatar random_sheet genome`, hashed in keyed mode. The 32-byte key
//! is the seed and then `g`, each as a little-endian u64, followed by 16 zero
//! bytes. A genome takes as many output bytes as its type reads: 1,024 for
//! classic, 16,384 for every other type. The same arguments therefore draw
//! the same sheet on every machine.
//!
//! Next to the PNG, a `.tsv` of the same name lists each cell's index, its
//! genome index, its mood and the genome's first 16 hex characters. With
//! `--genome-out DIR`, each cell's whole genome is also written to
//! `DIR/<cell>.hex`, so a chosen cell can be drawn again on its own, for
//! example with `avatar-export --genome-file`.

use std::{
    fmt::Write as _,
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
    process, thread,
};

use avatar::{render_image, Alpha, GroundShadow, ImageOptions, Mood, CLASSIC};
use avatar_svg::wild::mood;

const USAGE: &str = "usage: random_sheet --type TYPE [--version N] [--seed N] [--count N] \
                     [--columns N] [--cell PX] [--mood MOOD | --moods] \
                     [--genome-out DIR] --out PNG";
/// How many genomes `--moods` draws, each in every mood.
const MOOD_GENOMES: u64 = 4;
const SEPARATOR_GREY: u8 = 0xd0;
const LABEL_PAPER_GREY: u8 = 0xf0;
const LABEL_INK_GREY: u8 = 0x30;

/// The digits 0 to 9 on a 3 by 5 pixel grid. Each row is three bits, the
/// left pixel highest, and the top row is the highest three bits.
const DIGITS: [u16; 10] = [
    0b111_101_101_101_111,
    0b010_110_010_010_111,
    0b111_001_111_100_111,
    0b111_001_111_001_111,
    0b101_101_111_001_001,
    0b111_100_111_001_111,
    0b111_100_111_101_111,
    0b111_001_001_001_001,
    0b111_101_111_101_111,
    0b111_101_111_001_111,
];

struct Args {
    avatar_type: String,
    version: u32,
    seed: u64,
    count: u64,
    columns: usize,
    cell: u32,
    mood: Mood,
    moods: bool,
    genome_out: Option<PathBuf>,
    out: PathBuf,
}

fn main() {
    let args = parse_args();
    // Each cell draws one genome, by its index within the sheet, in one mood.
    let (cells, columns): (Vec<(u64, Mood)>, usize) = if args.moods {
        let cells = (0..MOOD_GENOMES).flat_map(|genome| mood::ALL.map(|mood| (genome, mood)));
        (cells.collect(), mood::ALL.len())
    } else {
        let cells = (0..args.count).map(|genome| (genome, args.mood));
        (cells.collect(), args.columns)
    };
    let genome_len = if args.avatar_type == CLASSIC {
        1_024
    } else {
        16_384
    };
    let genomes: Vec<Vec<u8>> = cells
        .iter()
        .map(|&(index, _)| genome(args.seed, index, genome_len))
        .collect();

    let images = render_cells(&args, &cells, &genomes);

    let cell = args.cell as usize;
    let width = columns * (cell + 1) + 1;
    let height = cells.len().div_ceil(columns) * (cell + 1) + 1;
    let mut sheet = vec![SEPARATOR_GREY; width * height * 3];
    // A digit is 4 scaled pixels wide with its gap, so 3 digits take about a
    // fifth of a 256 px cell.
    let scale = (cell / 80).max(1);
    for (index, pixels) in images.iter().enumerate() {
        let x0 = 1 + index % columns * (cell + 1);
        let y0 = 1 + index / columns * (cell + 1);
        for (y, row) in pixels.chunks_exact(cell * 4).enumerate() {
            let start = ((y0 + y) * width + x0) * 3;
            let (target, _) = sheet[start..start + cell * 3].as_chunks_mut::<3>();
            for (rgb, rgba) in target.iter_mut().zip(row.as_chunks::<4>().0) {
                // Premultiplied colour over white: c + (1 - alpha) * 255.
                let clear = 255 - rgba[3];
                for (out, &premultiplied) in rgb.iter_mut().zip(rgba) {
                    *out = premultiplied.saturating_add(clear);
                }
            }
        }
        draw_label(&mut sheet, width, x0, y0, index, scale);
    }

    if let Some(directory) = args.out.parent() {
        fs::create_dir_all(directory)
            .unwrap_or_else(|error| fail(&format!("create {}: {error}", directory.display())));
    }
    write_png(&args.out, width, height, &sheet);

    let mut tsv = format!(
        "# {} version {} seed {}\ncell\tgenome\tmood\tgenome_hex16\n",
        args.avatar_type, args.version, args.seed
    );
    for (index, (&(genome_index, mood), genome)) in cells.iter().zip(&genomes).enumerate() {
        write!(tsv, "{index}\t{genome_index}\t{}\t", mood::name(mood)).expect("write to String");
        for byte in &genome[..8] {
            write!(tsv, "{byte:02x}").expect("write to String");
        }
        tsv.push('\n');
    }
    if let Some(directory) = &args.genome_out {
        fs::create_dir_all(directory)
            .unwrap_or_else(|error| fail(&format!("create {}: {error}", directory.display())));
        for (index, genome) in genomes.iter().enumerate() {
            let mut hex = String::with_capacity(genome.len() * 2 + 1);
            for byte in genome {
                write!(hex, "{byte:02x}").expect("write to String");
            }
            hex.push('\n');
            let path = directory.join(format!("{index}.hex"));
            fs::write(&path, hex)
                .unwrap_or_else(|error| fail(&format!("write {}: {error}", path.display())));
        }
    }

    let tsv_path = args.out.with_extension("tsv");
    fs::write(&tsv_path, tsv)
        .unwrap_or_else(|error| fail(&format!("write {}: {error}", tsv_path.display())));
    eprintln!("wrote {} ({} cells)", args.out.display(), cells.len());
}

fn parse_args() -> Args {
    let mut args = Args {
        avatar_type: String::new(),
        version: 1,
        seed: 1,
        count: 16,
        columns: 4,
        cell: 256,
        mood: Mood::Neutral,
        moods: false,
        genome_out: None,
        out: PathBuf::new(),
    };
    let mut words = std::env::args().skip(1);
    while let Some(flag) = words.next() {
        if flag == "--moods" {
            args.moods = true;
            continue;
        }
        let value = words
            .next()
            .unwrap_or_else(|| fail(&format!("{flag} needs a value\n{USAGE}")));
        match flag.as_str() {
            "--type" => args.avatar_type = value,
            "--version" => args.version = number(&flag, &value),
            "--seed" => args.seed = number(&flag, &value),
            "--count" => args.count = number(&flag, &value),
            "--columns" => args.columns = number(&flag, &value),
            "--cell" => args.cell = number(&flag, &value),
            "--mood" => {
                args.mood = mood::ALL
                    .into_iter()
                    .find(|&mood| mood::name(mood) == value)
                    .unwrap_or_else(|| fail(&format!("unknown mood {value:?}")));
            }
            "--genome-out" => args.genome_out = Some(PathBuf::from(value)),
            "--out" => args.out = PathBuf::from(value),
            _ => fail(&format!("unknown argument {flag:?}\n{USAGE}")),
        }
    }
    if args.avatar_type.is_empty()
        || args.out.as_os_str().is_empty()
        || args.count == 0
        || args.columns == 0
    {
        fail(USAGE);
    }
    args
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> T {
    value
        .parse()
        .unwrap_or_else(|_| fail(&format!("{flag}: {value:?} is not a whole number")))
}

fn fail(message: &str) -> ! {
    eprintln!("random_sheet: {message}");
    process::exit(1)
}

/// Genome `index` of the sheet drawn with `seed`, as the module comment
/// describes.
fn genome(seed: u64, index: u64, len: usize) -> Vec<u8> {
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&index.to_le_bytes());
    let mut hasher = blake3::Hasher::new_keyed(&key);
    hasher.update(b"avatar random_sheet genome");
    let mut bytes = vec![0; len];
    hasher.finalize_xof().fill(&mut bytes);
    bytes
}

/// Renders every cell's RGBA pixels, in cell order, spread over the
/// machine's cores.
fn render_cells(args: &Args, cells: &[(u64, Mood)], genomes: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let options = ImageOptions {
        ground_shadow: GroundShadow::Keep,
        alpha: Alpha::Premultiplied,
    };
    let workers = thread::available_parallelism().map_or(1, usize::from);
    let chunk = cells.len().div_ceil(workers);
    let jobs: Vec<(usize, &(u64, Mood))> = cells.iter().enumerate().collect();
    thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|chunk| {
                let options = &options;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&(index, &(genome_index, mood))| {
                            render_image(
                                &genomes[index],
                                &args.avatar_type,
                                args.version,
                                mood,
                                args.cell,
                                options,
                            )
                            .unwrap_or_else(|error| {
                                fail(&format!("cell {index} (genome {genome_index}): {error}"))
                            })
                            .pixels
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a render thread panicked"))
            .collect()
    })
}

/// Draws `number` in pixel digits on a pale box at the cell's top left.
fn draw_label(sheet: &mut [u8], width: usize, x0: usize, y0: usize, number: usize, scale: usize) {
    let text = number.to_string();
    let paper = ((text.len() * 4 + 1) * scale, 7 * scale);
    fill(sheet, width, (x0, y0), paper, LABEL_PAPER_GREY);
    for (place, digit) in text.bytes().enumerate() {
        let bits = DIGITS[usize::from(digit - b'0')];
        for row in 0..5 {
            for column in 0..3 {
                if bits >> (14 - row * 3 - column) & 1 == 1 {
                    let x = x0 + (1 + place * 4 + column) * scale;
                    let y = y0 + (1 + row) * scale;
                    fill(sheet, width, (x, y), (scale, scale), LABEL_INK_GREY);
                }
            }
        }
    }
}

/// Paints a `size` rectangle of `grey` with its top left at `origin`.
fn fill(sheet: &mut [u8], width: usize, origin: (usize, usize), size: (usize, usize), grey: u8) {
    let (x0, y0) = origin;
    for y in y0..y0 + size.1 {
        sheet[(y * width + x0) * 3..(y * width + x0 + size.0) * 3].fill(grey);
    }
}

/// Writes the sheet as an 8-bit RGB PNG.
fn write_png(path: &Path, width: usize, height: usize, rgb: &[u8]) {
    let file = fs::File::create(path)
        .unwrap_or_else(|error| fail(&format!("create {}: {error}", path.display())));
    let size = |edge: usize| u32::try_from(edge).unwrap_or_else(|_| fail("the sheet is too large"));
    let mut encoder = png::Encoder::new(BufWriter::new(file), size(width), size(height));
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let written = encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgb).and_then(|()| writer.finish()));
    written.unwrap_or_else(|error| fail(&format!("write {}: {error}", path.display())));
}
