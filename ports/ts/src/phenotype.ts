export type Milli = bigint;
export type Q32 = bigint;
export type Bam16 = bigint;

export interface Rgb {
  r: bigint;
  g: bigint;
  b: bigint;
}

export interface Phenotype {
  genome_version: bigint;
  body: Body;
  palette: Palette;
  face: Face;
  pattern: Pattern;
  animation: Animation;
  mood_response: MoodResponse;
  asymmetry: Asymmetry;
  layers: Layers;
}

export interface Body {
  point_count: bigint;
  size: Milli;
  points: BodyPoint[];
}

export interface BodyPoint {
  x: Milli;
  y: Milli;
  tension: Q32;
}

export interface Palette {
  back: Rgb;
  primary: Rgb;
  primary_lighter: Rgb;
  primary_darkish: Rgb;
  primary_darker: Rgb;
  primary_darkest: Rgb;
  harmonic1: Rgb;
  harmonic1_darkish: Rgb;
  harmonic1_darkest: Rgb;
  harmonic2: Rgb;
  harmonic3: Rgb;
  harmonic4: Rgb;
  secondary_darker: Rgb;
  dark: Rgb;
  white: Rgb;
}

export interface Face {
  eyes: Eyes;
  brows: Brows;
  lashes: Lashes;
  mouth: Mouth;
  placement: FacePlacement;
}

export interface Eyes {
  position_mode: bigint;
  size_h: Q32;
  ipd: Q32;
  height_upper: Q32;
  height_lower: Q32;
  height_bias_gate: boolean;
  height_bias: Q32;
  vertical_pull_gate: boolean;
  vertical_pull: Q32;
  contour_inset1: Q32;
  contour_inset2: Q32;
  eyeball_radius_scale: Q32;
  pupil_scale: Q32;
  iris_shade_opacity: Q32;
  iris_shade_reduce: Q32;
  iris_saturation_reduce: Q32;
  lid_close_upper_neutral: Q32;
  lid_close_lower_neutral: Q32;
  lid_extension_gate: bigint;
  lid_extension_side: bigint;
  noise_freq_h: Q32;
  noise_freq_v: Q32;
  noise_seed: bigint;
  rotation_seed: bigint;
  lid_color_gene: bigint;
  iris_color_gene: bigint;
}

export interface Brows {
  end_height1: Q32;
  mid_x: Q32;
  end_height2: Q32;
  tensions: Q32[];
  width_coupling: Q32;
  clearance: Q32;
}

export interface Lashes {
  count: bigint;
  removed: bigint;
  length_scale: Q32;
  stroke_scale: Q32;
  direction: bigint;
}

export interface Mouth {
  family: bigint;
  width_scale_gate: boolean;
  width_scale: Q32;
  aspect_widen_gate: boolean;
  params: Q32[];
}

export interface FacePlacement {
  width_coeff: Q32;
  height_coeff: Q32;
}

export interface Pattern {
  class: bigint;
  circler: Circler;
  voronoi: Voronoi;
  cut: Cut;
}

export interface Circler {
  point_reduce: bigint;
  size: Q32;
  y_offset_div: Q32;
  tension: Q32;
  pull_min: Q32;
}

export interface Voronoi {
  index: bigint;
  rotation: Bam16;
  scale: Q32;
}

export interface Cut {
  spread: Q32;
  y_adjust_div: Q32;
}

export interface Animation {
  breath_period: bigint;
  breath_amplitude: Q32;
  blink_interval: bigint;
  blink_jitter_seed: bigint;
  mood_transition_tempo: Q32;
}

export interface MoodResponse {
  lid_reactivity: Q32;
  brow_reactivity: Q32;
  eye_rotation_reactivity: Q32;
  mouth_reactivity: Q32;
}

export interface Asymmetry {
  deltas: Q32[];
}

export interface Layers {
  count: bigint;
  params: Q32[];
}

type CanonicalValue = bigint | boolean | CanonicalObject | CanonicalValue[];
type CanonicalObject = { [key: string]: CanonicalValue };
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/**
 * Serializes the integer-only phenotype schema into its canonical byte form.
 * Keys are sorted here rather than delegated to JSON.stringify because byte
 * equality is the boundary contract and BigInt must never pass through a float.
 */
export function toCanonicalJson(phenotype: Phenotype): Uint8Array {
  return encoder.encode(writeCanonical(phenotype as unknown as CanonicalObject));
}

const canonicalKeyOrders = [
  ["animation", "asymmetry", "body", "face", "genome_version", "layers", "mood_response", "palette", "pattern"],
  ["blink_interval", "blink_jitter_seed", "breath_amplitude", "breath_period", "mood_transition_tempo"],
  ["deltas"],
  ["point_count", "points", "size"],
  ["tension", "x", "y"],
  ["back", "dark", "harmonic1", "harmonic1_darkish", "harmonic1_darkest", "harmonic2", "harmonic3", "harmonic4", "primary", "primary_darker", "primary_darkest", "primary_darkish", "primary_lighter", "secondary_darker", "white"],
  ["b", "g", "r"],
  ["brows", "eyes", "lashes", "mouth", "placement"],
  ["contour_inset1", "contour_inset2", "eyeball_radius_scale", "height_bias", "height_bias_gate", "height_lower", "height_upper", "ipd", "iris_color_gene", "iris_saturation_reduce", "iris_shade_opacity", "iris_shade_reduce", "lid_close_lower_neutral", "lid_close_upper_neutral", "lid_color_gene", "lid_extension_gate", "lid_extension_side", "noise_freq_h", "noise_freq_v", "noise_seed", "position_mode", "pupil_scale", "rotation_seed", "size_h", "vertical_pull", "vertical_pull_gate"],
  ["clearance", "end_height1", "end_height2", "mid_x", "tensions", "width_coupling"],
  ["count", "direction", "length_scale", "removed", "stroke_scale"],
  ["aspect_widen_gate", "family", "params", "width_scale", "width_scale_gate"],
  ["height_coeff", "width_coeff"],
  ["circler", "class", "cut", "voronoi"],
  ["point_reduce", "pull_min", "size", "tension", "y_offset_div"],
  ["index", "rotation", "scale"],
  ["spread", "y_adjust_div"],
  ["brow_reactivity", "eye_rotation_reactivity", "lid_reactivity", "mouth_reactivity"],
  ["count", "params"],
] as const;

const canonicalOrdersByKeySet = new Map(
  canonicalKeyOrders.map((order) => [[...order].sort().join("\u0000"), order]),
);

function writeCanonical(value: CanonicalValue): string {
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (typeof value === "boolean") {
    return value ? "1" : "0";
  }
  if (Array.isArray(value)) {
    return `[${value.map(writeCanonical).join(",")}]`;
  }
  const keys = Object.keys(value);
  const order = canonicalOrdersByKeySet.get([...keys].sort().join("\u0000")) ?? keys.sort();
  return `{${order.map((key) => `${JSON.stringify(key)}:${writeCanonical(value[key])}`).join(",")}}`;
}

export class PhenotypeParseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PhenotypeParseError";
  }
}

type JsonValue = bigint | boolean | null | string | JsonObject | JsonValue[];
interface JsonObject {
  [key: string]: JsonValue;
}

class IntegerJsonParser {
  private position = 0;

  constructor(private readonly source: string) {}

  parse(): JsonValue {
    this.whitespace();
    const value = this.value();
    this.whitespace();
    if (this.position !== this.source.length) {
      this.fail("unexpected trailing input");
    }
    return value;
  }

  private value(): JsonValue {
    const character = this.source[this.position];
    if (character === "{") return this.object();
    if (character === "[") return this.array();
    if (character === '"') return this.string();
    if (character === "-" || (character >= "0" && character <= "9")) return this.integer();
    if (this.source.startsWith("true", this.position)) {
      this.position += 4;
      return true;
    }
    if (this.source.startsWith("false", this.position)) {
      this.position += 5;
      return false;
    }
    if (this.source.startsWith("null", this.position)) {
      this.position += 4;
      return null;
    }
    this.fail("expected a JSON value");
  }

  private object(): JsonObject {
    this.position += 1;
    this.whitespace();
    const result: JsonObject = {};
    if (this.consume("}")) return result;
    while (true) {
      if (this.source[this.position] !== '"') this.fail("object key must be a string");
      const key = this.string();
      this.whitespace();
      this.expect(":");
      this.whitespace();
      if (Object.hasOwn(result, key)) this.fail(`duplicate object key ${key}`);
      result[key] = this.value();
      this.whitespace();
      if (this.consume("}")) return result;
      this.expect(",");
      this.whitespace();
    }
  }

  private array(): JsonValue[] {
    this.position += 1;
    this.whitespace();
    const result: JsonValue[] = [];
    if (this.consume("]")) return result;
    while (true) {
      result.push(this.value());
      this.whitespace();
      if (this.consume("]")) return result;
      this.expect(",");
      this.whitespace();
    }
  }

  private string(): string {
    const start = this.position;
    this.position += 1;
    while (this.position < this.source.length) {
      const code = this.source.charCodeAt(this.position);
      if (code < 0x20) this.fail("control character in string");
      if (this.source[this.position] === "\\") {
        this.position += 2;
        continue;
      }
      this.position += 1;
      if (this.source[this.position - 1] === '"') {
        try {
          return JSON.parse(this.source.slice(start, this.position)) as string;
        } catch {
          this.fail("invalid JSON string");
        }
      }
    }
    this.fail("unterminated string");
  }

  private integer(): bigint {
    const start = this.position;
    if (this.consume("-")) {
      if (!this.digit()) this.fail("invalid number");
    }
    if (this.consume("0")) {
      if (this.digit()) this.fail("leading zero in number");
    } else {
      if (!this.nonzeroDigit()) this.fail("invalid number");
      while (this.digit()) this.position += 1;
    }
    if (".eE".includes(this.source[this.position] ?? "")) {
      this.fail("phenotype numbers must be integers");
    }
    return BigInt(this.source.slice(start, this.position));
  }

  private whitespace(): void {
    while (this.position < this.source.length && " \t\n\r".includes(this.source[this.position])) {
      this.position += 1;
    }
  }

  private consume(character: string): boolean {
    if (this.source[this.position] !== character) return false;
    this.position += 1;
    return true;
  }

  private expect(character: string): void {
    if (!this.consume(character)) this.fail(`expected ${character}`);
  }

  private digit(): boolean {
    const character = this.source[this.position];
    return character >= "0" && character <= "9";
  }

  private nonzeroDigit(): boolean {
    const character = this.source[this.position];
    return character >= "1" && character <= "9";
  }

  private fail(message: string): never {
    throw new PhenotypeParseError(`${message} at byte ${this.position}`);
  }
}

/**
 * Parses a complete phenotype object while accepting harmless whitespace and key
 * order changes. Unknown, missing, floating, and out-of-range fields are rejected
 * so canonicalizing input cannot silently change the avatar it describes.
 */
export function fromCanonicalJson(input: Uint8Array | string): Phenotype {
  let source: string;
  try {
    source = typeof input === "string" ? input : decoder.decode(input);
  } catch {
    throw new PhenotypeParseError("input is not valid UTF-8");
  }
  return parsePhenotype(new IntegerJsonParser(source).parse());
}

function object(value: JsonValue, context: string): JsonObject {
  if (value === null || Array.isArray(value) || typeof value !== "object") {
    throw new PhenotypeParseError(`${context} must be an object`);
  }
  return value;
}

function exactKeys(record: JsonObject, keys: readonly string[], context: string): void {
  const actual = Object.keys(record).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new PhenotypeParseError(`${context} has unknown or missing fields`);
  }
}

function field(record: JsonObject, key: string, context: string): JsonValue {
  const value = record[key];
  if (value === undefined) throw new PhenotypeParseError(`${context}.${key} is missing`);
  return value;
}

function integer(value: JsonValue, context: string): bigint {
  if (typeof value !== "bigint") throw new PhenotypeParseError(`${context} must be an integer`);
  return value;
}

function ranged(value: JsonValue, low: bigint, high: bigint, context: string): bigint {
  const result = integer(value, context);
  if (result < low || result > high) throw new PhenotypeParseError(`${context} is out of range`);
  return result;
}

const i64 = (value: JsonValue, context: string) => ranged(value, -(1n << 63n), (1n << 63n) - 1n, context);
const u8 = (value: JsonValue, context: string) => ranged(value, 0n, 255n, context);
const u16 = (value: JsonValue, context: string) => ranged(value, 0n, 65535n, context);
const q32 = i64;
const milli = i64;

function gate(value: JsonValue, context: string): boolean {
  const result = integer(value, context);
  if (result !== 0n && result !== 1n) throw new PhenotypeParseError(`${context} must be 0 or 1`);
  return result === 1n;
}

function array<T>(value: JsonValue, length: number, context: string, parse: (item: JsonValue, index: number) => T): T[] {
  if (!Array.isArray(value)) throw new PhenotypeParseError(`${context} must be an array`);
  if (value.length !== length) throw new PhenotypeParseError(`${context} must contain ${length} items`);
  return value.map(parse);
}

function parsePhenotype(value: JsonValue): Phenotype {
  const record = object(value, "phenotype");
  exactKeys(record, ["genome_version", "body", "palette", "face", "pattern", "animation", "mood_response", "asymmetry", "layers"], "phenotype");
  return {
    genome_version: u16(field(record, "genome_version", "phenotype"), "genome_version"),
    body: parseBody(field(record, "body", "phenotype")),
    palette: parsePalette(field(record, "palette", "phenotype")),
    face: parseFace(field(record, "face", "phenotype")),
    pattern: parsePattern(field(record, "pattern", "phenotype")),
    animation: parseAnimation(field(record, "animation", "phenotype")),
    mood_response: parseMoodResponse(field(record, "mood_response", "phenotype")),
    asymmetry: parseAsymmetry(field(record, "asymmetry", "phenotype")),
    layers: parseLayers(field(record, "layers", "phenotype")),
  };
}

function parseBody(value: JsonValue): Body {
  const record = object(value, "body");
  exactKeys(record, ["point_count", "size", "points"], "body");
  return {
    point_count: u8(field(record, "point_count", "body"), "body.point_count"),
    size: milli(field(record, "size", "body"), "body.size"),
    points: array(field(record, "points", "body"), 16, "body.points", parseBodyPoint),
  };
}

function parseBodyPoint(value: JsonValue): BodyPoint {
  const record = object(value, "body point");
  exactKeys(record, ["x", "y", "tension"], "body point");
  return {
    x: milli(field(record, "x", "body point"), "body.points[].x"),
    y: milli(field(record, "y", "body point"), "body.points[].y"),
    tension: q32(field(record, "tension", "body point"), "body.points[].tension"),
  };
}

function parseRgb(value: JsonValue): Rgb {
  const record = object(value, "RGB color");
  exactKeys(record, ["r", "g", "b"], "RGB color");
  return {
    r: u8(field(record, "r", "RGB color"), "color.r"),
    g: u8(field(record, "g", "RGB color"), "color.g"),
    b: u8(field(record, "b", "RGB color"), "color.b"),
  };
}

function parsePalette(value: JsonValue): Palette {
  const record = object(value, "palette");
  const keys = ["back", "primary", "primary_lighter", "primary_darkish", "primary_darker", "primary_darkest", "harmonic1", "harmonic1_darkish", "harmonic1_darkest", "harmonic2", "harmonic3", "harmonic4", "secondary_darker", "dark", "white"] as const;
  exactKeys(record, keys, "palette");
  return Object.fromEntries(keys.map((key) => [key, parseRgb(field(record, key, "palette"))])) as unknown as Palette;
}

function parseFace(value: JsonValue): Face {
  const record = object(value, "face");
  exactKeys(record, ["eyes", "brows", "lashes", "mouth", "placement"], "face");
  return {
    eyes: parseEyes(field(record, "eyes", "face")),
    brows: parseBrows(field(record, "brows", "face")),
    lashes: parseLashes(field(record, "lashes", "face")),
    mouth: parseMouth(field(record, "mouth", "face")),
    placement: parseFacePlacement(field(record, "placement", "face")),
  };
}

function parseEyes(value: JsonValue): Eyes {
  const record = object(value, "eyes");
  const q32Keys = ["size_h", "ipd", "height_upper", "height_lower", "height_bias", "vertical_pull", "contour_inset1", "contour_inset2", "eyeball_radius_scale", "pupil_scale", "iris_shade_opacity", "iris_shade_reduce", "iris_saturation_reduce", "lid_close_upper_neutral", "lid_close_lower_neutral", "noise_freq_h", "noise_freq_v"] as const;
  const byteKeys = ["position_mode", "lid_extension_gate", "lid_extension_side", "noise_seed", "rotation_seed", "lid_color_gene", "iris_color_gene"] as const;
  const gateKeys = ["height_bias_gate", "vertical_pull_gate"] as const;
  exactKeys(record, [...q32Keys, ...byteKeys, ...gateKeys], "eyes");
  const parsed: Record<string, bigint | boolean | number> = Object.fromEntries(
    q32Keys.map((key) => [key, q32(field(record, key, "eyes"), `eyes.${key}`)]),
  );
  for (const key of byteKeys) parsed[key] = u8(field(record, key, "eyes"), `eyes.${key}`);
  for (const key of gateKeys) parsed[key] = gate(field(record, key, "eyes"), `eyes.${key}`);
  return parsed as unknown as Eyes;
}

function parseBrows(value: JsonValue): Brows {
  const record = object(value, "brows");
  exactKeys(record, ["end_height1", "mid_x", "end_height2", "tensions", "width_coupling", "clearance"], "brows");
  return {
    end_height1: q32(field(record, "end_height1", "brows"), "brows.end_height1"),
    mid_x: q32(field(record, "mid_x", "brows"), "brows.mid_x"),
    end_height2: q32(field(record, "end_height2", "brows"), "brows.end_height2"),
    tensions: array(field(record, "tensions", "brows"), 3, "brows.tensions", (item) => q32(item, "brows.tensions[]")),
    width_coupling: q32(field(record, "width_coupling", "brows"), "brows.width_coupling"),
    clearance: q32(field(record, "clearance", "brows"), "brows.clearance"),
  };
}

function parseLashes(value: JsonValue): Lashes {
  const record = object(value, "lashes");
  exactKeys(record, ["count", "removed", "length_scale", "stroke_scale", "direction"], "lashes");
  return {
    count: u8(field(record, "count", "lashes"), "lashes.count"),
    removed: u8(field(record, "removed", "lashes"), "lashes.removed"),
    length_scale: q32(field(record, "length_scale", "lashes"), "lashes.length_scale"),
    stroke_scale: q32(field(record, "stroke_scale", "lashes"), "lashes.stroke_scale"),
    direction: u8(field(record, "direction", "lashes"), "lashes.direction"),
  };
}

function parseMouth(value: JsonValue): Mouth {
  const record = object(value, "mouth");
  exactKeys(record, ["family", "width_scale_gate", "width_scale", "aspect_widen_gate", "params"], "mouth");
  return {
    family: u8(field(record, "family", "mouth"), "mouth.family"),
    width_scale_gate: gate(field(record, "width_scale_gate", "mouth"), "mouth.width_scale_gate"),
    width_scale: q32(field(record, "width_scale", "mouth"), "mouth.width_scale"),
    aspect_widen_gate: gate(field(record, "aspect_widen_gate", "mouth"), "mouth.aspect_widen_gate"),
    params: array(field(record, "params", "mouth"), 8, "mouth.params", (item) => q32(item, "mouth.params[]")),
  };
}

function parseFacePlacement(value: JsonValue): FacePlacement {
  const record = object(value, "face placement");
  exactKeys(record, ["width_coeff", "height_coeff"], "face placement");
  return {
    width_coeff: q32(field(record, "width_coeff", "face placement"), "placement.width_coeff"),
    height_coeff: q32(field(record, "height_coeff", "face placement"), "placement.height_coeff"),
  };
}

function parsePattern(value: JsonValue): Pattern {
  const record = object(value, "pattern");
  exactKeys(record, ["class", "circler", "voronoi", "cut"], "pattern");
  return {
    class: u8(field(record, "class", "pattern"), "pattern.class"),
    circler: parseCircler(field(record, "circler", "pattern")),
    voronoi: parseVoronoi(field(record, "voronoi", "pattern")),
    cut: parseCut(field(record, "cut", "pattern")),
  };
}

function parseCircler(value: JsonValue): Circler {
  const record = object(value, "circler pattern");
  exactKeys(record, ["point_reduce", "size", "y_offset_div", "tension", "pull_min"], "circler pattern");
  return {
    point_reduce: u8(field(record, "point_reduce", "circler pattern"), "circler.point_reduce"),
    size: q32(field(record, "size", "circler pattern"), "circler.size"),
    y_offset_div: q32(field(record, "y_offset_div", "circler pattern"), "circler.y_offset_div"),
    tension: q32(field(record, "tension", "circler pattern"), "circler.tension"),
    pull_min: q32(field(record, "pull_min", "circler pattern"), "circler.pull_min"),
  };
}

function parseVoronoi(value: JsonValue): Voronoi {
  const record = object(value, "voronoi pattern");
  exactKeys(record, ["index", "rotation", "scale"], "voronoi pattern");
  return {
    index: u8(field(record, "index", "voronoi pattern"), "voronoi.index"),
    rotation: u16(field(record, "rotation", "voronoi pattern"), "voronoi.rotation"),
    scale: q32(field(record, "scale", "voronoi pattern"), "voronoi.scale"),
  };
}

function parseCut(value: JsonValue): Cut {
  const record = object(value, "cut pattern");
  exactKeys(record, ["spread", "y_adjust_div"], "cut pattern");
  return {
    spread: q32(field(record, "spread", "cut pattern"), "cut.spread"),
    y_adjust_div: q32(field(record, "y_adjust_div", "cut pattern"), "cut.y_adjust_div"),
  };
}

function parseAnimation(value: JsonValue): Animation {
  const record = object(value, "animation");
  exactKeys(record, ["breath_period", "breath_amplitude", "blink_interval", "blink_jitter_seed", "mood_transition_tempo"], "animation");
  return {
    breath_period: u16(field(record, "breath_period", "animation"), "animation.breath_period"),
    breath_amplitude: q32(field(record, "breath_amplitude", "animation"), "animation.breath_amplitude"),
    blink_interval: u16(field(record, "blink_interval", "animation"), "animation.blink_interval"),
    blink_jitter_seed: u8(field(record, "blink_jitter_seed", "animation"), "animation.blink_jitter_seed"),
    mood_transition_tempo: q32(field(record, "mood_transition_tempo", "animation"), "animation.mood_transition_tempo"),
  };
}

function parseMoodResponse(value: JsonValue): MoodResponse {
  const record = object(value, "mood response");
  exactKeys(record, ["lid_reactivity", "brow_reactivity", "eye_rotation_reactivity", "mouth_reactivity"], "mood response");
  return {
    lid_reactivity: q32(field(record, "lid_reactivity", "mood response"), "mood_response.lid_reactivity"),
    brow_reactivity: q32(field(record, "brow_reactivity", "mood response"), "mood_response.brow_reactivity"),
    eye_rotation_reactivity: q32(field(record, "eye_rotation_reactivity", "mood response"), "mood_response.eye_rotation_reactivity"),
    mouth_reactivity: q32(field(record, "mouth_reactivity", "mood response"), "mood_response.mouth_reactivity"),
  };
}

function parseAsymmetry(value: JsonValue): Asymmetry {
  const record = object(value, "asymmetry");
  exactKeys(record, ["deltas"], "asymmetry");
  return { deltas: array(field(record, "deltas", "asymmetry"), 8, "asymmetry.deltas", (item) => q32(item, "asymmetry.deltas[]")) };
}

function parseLayers(value: JsonValue): Layers {
  const record = object(value, "layers");
  exactKeys(record, ["count", "params"], "layers");
  return {
    count: u8(field(record, "count", "layers"), "layers.count"),
    params: array(field(record, "params", "layers"), 16, "layers.params", (item) => q32(item, "layers.params[]")),
  };
}
