/** A coordinate in milliunits, the render tree's integer geometry unit. */
export type Milli = bigint;

/** A raw signed Q32.32 fixed-point value. */
export type Q32 = bigint;

/** A binary angle measurement stored in an unsigned 16-bit word. */
export type Bam16 = number;

/** An 8-bit RGB color. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** A complete avatar drawing in painter's order. */
export interface RenderTree {
  canvas: number;
  background: Rgb | null;
  primitives: Primitive[];
}

/**
 * One independently drawable item with fully resolved transforms.
 * Labels are diagnostic names without a stability promise; roles are the
 * semantic contract consumers can use to change drawing behavior.
 */
export interface Primitive {
  shape: Shape;
  transform: Transform | null;
  paint: Paint;
  stroke: Stroke | null;
  opacity: Q32 | null;
  clip: Shape | null;
  effects: Effect[];
  label: string;
  role: Role | null;
}

export type Role = "ground_shadow";

export type FillRule = "non_zero" | "even_odd";

/** One absolute path segment in canonical milliunits. */
export type Segment =
  | { kind: "move_to"; x: Milli; y: Milli }
  | {
      kind: "cubic_to";
      cp1x: Milli;
      cp1y: Milli;
      cp2x: Milli;
      cp2y: Milli;
      x: Milli;
      y: Milli;
    }
  | { kind: "line_to"; x: Milli; y: Milli };

/** Drawable geometry in pre-transform canonical milliunits. */
export type Shape =
  | { kind: "path"; segments: Segment[]; closed: boolean; fill_rule: FillRule }
  | { kind: "circle"; cx: Milli; cy: Milli; r: Milli }
  | { kind: "ellipse"; cx: Milli; cy: Milli; rx: Milli; ry: Milli }
  | { kind: "rect"; x: Milli; y: Milli; w: Milli; h: Milli };

/** One radial-gradient stop. */
export interface GradientStop {
  offset_q32: Q32;
  color: Rgb;
}

/** Fill paint for a primitive. */
export type Paint =
  | { kind: "solid"; color: Rgb }
  | { kind: "radial_gradient"; stops: GradientStop[] }
  | { kind: "none" };

export type StrokeCap = "butt" | "round" | "square";
export type StrokeJoin = "miter" | "round" | "bevel";

/** Per-primitive stroke styling. */
export interface Stroke {
  color: Rgb;
  width: Milli;
  cap: StrokeCap;
  join: StrokeJoin;
}

/** Optional rendering behavior attached to a drawable primitive. */
export type Effect =
  | { kind: "lit"; light_y: Milli }
  | { kind: "masked"; mask: Primitive[] }
  | {
      kind: "turbulence";
      seed: number;
      freq_h: Q32;
      freq_v: Q32;
      octaves: number;
      scale: Q32;
    };

/** One transform operation in serializer order. */
export type TransformOp =
  | { kind: "translate"; tx: Milli; ty: Milli }
  | { kind: "rotate"; angle: Bam16; cx: Milli; cy: Milli }
  | { kind: "scale"; sx: Q32; sy: Q32 };

/** Q32.32 affine coefficients with milliunit translation. */
export interface Affine {
  a: Q32;
  b: Q32;
  c: Q32;
  d: Q32;
  tx: Milli;
  ty: Milli;
}

/** A serializer-order transform operation chain. */
export interface Transform {
  ops: TransformOp[];
}

type JsonValue =
  | { type: "null" }
  | { type: "boolean"; value: boolean }
  | { type: "integer"; value: bigint }
  | { type: "string"; value: string }
  | { type: "array"; values: JsonValue[] }
  | { type: "object"; fields: Map<string, JsonValue> };

const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const U8_MAX = (1n << 8n) - 1n;
const U16_MAX = (1n << 16n) - 1n;
const U32_MAX = (1n << 32n) - 1n;

/**
 * Decodes the canonical render-tree JSON grammar without allowing JavaScript
 * number conversion to round a fixed-point value before its range is checked.
 */
export function decodeRenderTreeJson(bytes: Uint8Array | string): RenderTree {
  const source =
    typeof bytes === "string" ? bytes : new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  return decodeRenderTree(new JsonParser(source).parseDocument(), "render tree");
}

function decodeRenderTree(value: JsonValue, path: string): RenderTree {
  const fields = object(value, path, ["background", "canvas", "primitives"]);
  return {
    background: nullable(fields, "background", path, decodeRgb),
    canvas: unsignedInteger(required(fields, "canvas", path), `${path}.canvas`, U32_MAX),
    primitives: array(required(fields, "primitives", path), `${path}.primitives`, decodePrimitive),
  };
}

function decodePrimitive(value: JsonValue, path: string): Primitive {
  const fields = object(value, path, ["clip", "effects", "label", "opacity", "paint", "role", "shape", "stroke", "transform"]);
  return {
    clip: nullable(fields, "clip", path, decodeShape),
    effects: array(required(fields, "effects", path), `${path}.effects`, decodeEffect),
    label: string(required(fields, "label", path), `${path}.label`),
    opacity: nullable(fields, "opacity", path, integer64),
    paint: decodePaint(required(fields, "paint", path), `${path}.paint`),
    role: nullable(fields, "role", path, decodeRole),
    shape: decodeShape(required(fields, "shape", path), `${path}.shape`),
    stroke: nullable(fields, "stroke", path, decodeStroke),
    transform: nullable(fields, "transform", path, decodeTransform),
  };
}

function decodeRole(value: JsonValue, path: string): Role {
  const role = string(value, path);
  if (role !== "ground_shadow") throw new Error(`${path}: unknown role ${role}`);
  return role;
}

function decodeShape(value: JsonValue, path: string): Shape {
  const fields = rawObject(value, path);
  const kind = string(required(fields, "kind", path), `${path}.kind`);
  switch (kind) {
    case "path":
      requireExactKeys(fields, ["closed", "fill_rule", "kind", "segments"], path);
      return {
        kind,
        closed: boolean(required(fields, "closed", path), `${path}.closed`),
        fill_rule: decodeFillRule(required(fields, "fill_rule", path), `${path}.fill_rule`),
        segments: array(required(fields, "segments", path), `${path}.segments`, decodeSegment),
      };
    case "circle":
      requireExactKeys(fields, ["cx", "cy", "kind", "r"], path);
      return {
        kind,
        cx: integer64(required(fields, "cx", path), `${path}.cx`),
        cy: integer64(required(fields, "cy", path), `${path}.cy`),
        r: integer64(required(fields, "r", path), `${path}.r`),
      };
    case "ellipse":
      requireExactKeys(fields, ["cx", "cy", "kind", "rx", "ry"], path);
      return {
        kind,
        cx: integer64(required(fields, "cx", path), `${path}.cx`),
        cy: integer64(required(fields, "cy", path), `${path}.cy`),
        rx: integer64(required(fields, "rx", path), `${path}.rx`),
        ry: integer64(required(fields, "ry", path), `${path}.ry`),
      };
    case "rect":
      requireExactKeys(fields, ["h", "kind", "w", "x", "y"], path);
      return {
        kind,
        h: integer64(required(fields, "h", path), `${path}.h`),
        w: integer64(required(fields, "w", path), `${path}.w`),
        x: integer64(required(fields, "x", path), `${path}.x`),
        y: integer64(required(fields, "y", path), `${path}.y`),
      };
    default:
      fail(`${path}.kind: unknown shape tag ${JSON.stringify(kind)}`);
  }
}

function decodeSegment(value: JsonValue, path: string): Segment {
  const fields = rawObject(value, path);
  const kind = string(required(fields, "kind", path), `${path}.kind`);
  switch (kind) {
    case "move_to":
      requireExactKeys(fields, ["kind", "x", "y"], path);
      return {
        kind,
        x: integer64(required(fields, "x", path), `${path}.x`),
        y: integer64(required(fields, "y", path), `${path}.y`),
      };
    case "cubic_to":
      requireExactKeys(fields, ["cp1x", "cp1y", "cp2x", "cp2y", "kind", "x", "y"], path);
      return {
        kind,
        cp1x: integer64(required(fields, "cp1x", path), `${path}.cp1x`),
        cp1y: integer64(required(fields, "cp1y", path), `${path}.cp1y`),
        cp2x: integer64(required(fields, "cp2x", path), `${path}.cp2x`),
        cp2y: integer64(required(fields, "cp2y", path), `${path}.cp2y`),
        x: integer64(required(fields, "x", path), `${path}.x`),
        y: integer64(required(fields, "y", path), `${path}.y`),
      };
    case "line_to":
      requireExactKeys(fields, ["kind", "x", "y"], path);
      return {
        kind,
        x: integer64(required(fields, "x", path), `${path}.x`),
        y: integer64(required(fields, "y", path), `${path}.y`),
      };
    default:
      fail(`${path}.kind: unknown segment tag ${JSON.stringify(kind)}`);
  }
}

function decodeFillRule(value: JsonValue, path: string): FillRule {
  const tag = string(value, path);
  if (tag === "non_zero" || tag === "even_odd") return tag;
  return fail(`${path}: unknown fill-rule tag ${JSON.stringify(tag)}`);
}

function decodeRgb(value: JsonValue, path: string): Rgb {
  const fields = object(value, path, ["b", "g", "r"]);
  return {
    b: unsignedInteger(required(fields, "b", path), `${path}.b`, U8_MAX),
    g: unsignedInteger(required(fields, "g", path), `${path}.g`, U8_MAX),
    r: unsignedInteger(required(fields, "r", path), `${path}.r`, U8_MAX),
  };
}

function decodePaint(value: JsonValue, path: string): Paint {
  const fields = rawObject(value, path);
  const kind = string(required(fields, "kind", path), `${path}.kind`);
  switch (kind) {
    case "solid":
      requireExactKeys(fields, ["color", "kind"], path);
      return { kind, color: decodeRgb(required(fields, "color", path), `${path}.color`) };
    case "radial_gradient":
      requireExactKeys(fields, ["kind", "stops"], path);
      return {
        kind,
        stops: array(required(fields, "stops", path), `${path}.stops`, decodeGradientStop),
      };
    case "none":
      requireExactKeys(fields, ["kind"], path);
      return { kind };
    default:
      fail(`${path}.kind: unknown paint tag ${JSON.stringify(kind)}`);
  }
}

function decodeGradientStop(value: JsonValue, path: string): GradientStop {
  const fields = object(value, path, ["color", "offset_q32"]);
  return {
    color: decodeRgb(required(fields, "color", path), `${path}.color`),
    offset_q32: integer64(required(fields, "offset_q32", path), `${path}.offset_q32`),
  };
}

function decodeStroke(value: JsonValue, path: string): Stroke {
  const fields = object(value, path, ["cap", "color", "join", "width"]);
  return {
    cap: decodeStrokeCap(required(fields, "cap", path), `${path}.cap`),
    color: decodeRgb(required(fields, "color", path), `${path}.color`),
    join: decodeStrokeJoin(required(fields, "join", path), `${path}.join`),
    width: integer64(required(fields, "width", path), `${path}.width`),
  };
}

function decodeStrokeCap(value: JsonValue, path: string): StrokeCap {
  const tag = string(value, path);
  if (tag === "butt" || tag === "round" || tag === "square") return tag;
  return fail(`${path}: unknown stroke-cap tag ${JSON.stringify(tag)}`);
}

function decodeStrokeJoin(value: JsonValue, path: string): StrokeJoin {
  const tag = string(value, path);
  if (tag === "miter" || tag === "round" || tag === "bevel") return tag;
  return fail(`${path}: unknown stroke-join tag ${JSON.stringify(tag)}`);
}

function decodeEffect(value: JsonValue, path: string): Effect {
  const fields = rawObject(value, path);
  const kind = string(required(fields, "kind", path), `${path}.kind`);
  switch (kind) {
    case "lit":
      requireExactKeys(fields, ["kind", "light_y"], path);
      return { kind, light_y: integer64(required(fields, "light_y", path), `${path}.light_y`) };
    case "masked":
      requireExactKeys(fields, ["kind", "mask"], path);
      return { kind, mask: array(required(fields, "mask", path), `${path}.mask`, decodePrimitive) };
    case "turbulence":
      requireExactKeys(fields, ["freq_h", "freq_v", "kind", "octaves", "scale", "seed"], path);
      return {
        kind,
        freq_h: integer64(required(fields, "freq_h", path), `${path}.freq_h`),
        freq_v: integer64(required(fields, "freq_v", path), `${path}.freq_v`),
        octaves: unsignedInteger(required(fields, "octaves", path), `${path}.octaves`, U8_MAX),
        scale: integer64(required(fields, "scale", path), `${path}.scale`),
        seed: unsignedInteger(required(fields, "seed", path), `${path}.seed`, U8_MAX),
      };
    default:
      fail(`${path}.kind: unknown effect tag ${JSON.stringify(kind)}`);
  }
}

function decodeTransform(value: JsonValue, path: string): Transform {
  const fields = object(value, path, ["ops"]);
  return { ops: array(required(fields, "ops", path), `${path}.ops`, decodeTransformOp) };
}

function decodeTransformOp(value: JsonValue, path: string): TransformOp {
  const fields = rawObject(value, path);
  const kind = string(required(fields, "kind", path), `${path}.kind`);
  switch (kind) {
    case "translate":
      requireExactKeys(fields, ["kind", "tx", "ty"], path);
      return {
        kind,
        tx: integer64(required(fields, "tx", path), `${path}.tx`),
        ty: integer64(required(fields, "ty", path), `${path}.ty`),
      };
    case "rotate":
      requireExactKeys(fields, ["angle", "cx", "cy", "kind"], path);
      return {
        kind,
        angle: unsignedInteger(required(fields, "angle", path), `${path}.angle`, U16_MAX),
        cx: integer64(required(fields, "cx", path), `${path}.cx`),
        cy: integer64(required(fields, "cy", path), `${path}.cy`),
      };
    case "scale":
      requireExactKeys(fields, ["kind", "sx", "sy"], path);
      return {
        kind,
        sx: integer64(required(fields, "sx", path), `${path}.sx`),
        sy: integer64(required(fields, "sy", path), `${path}.sy`),
      };
    default:
      fail(`${path}.kind: unknown transform-operation tag ${JSON.stringify(kind)}`);
  }
}

function rawObject(value: JsonValue, path: string): Map<string, JsonValue> {
  if (value.type === "object") return value.fields;
  return fail(`${path}: expected an object`);
}

function object(value: JsonValue, path: string, keys: readonly string[]): Map<string, JsonValue> {
  const fields = rawObject(value, path);
  requireExactKeys(fields, keys, path);
  return fields;
}

function requireExactKeys(fields: Map<string, JsonValue>, keys: readonly string[], path: string): void {
  const allowed = new Set(keys);
  for (const key of fields.keys()) {
    if (!allowed.has(key)) fail(`${path}: unknown field ${JSON.stringify(key)}`);
  }
  for (const key of keys) {
    if (!fields.has(key)) fail(`${path}: missing required field ${JSON.stringify(key)}`);
  }
}

function required(fields: Map<string, JsonValue>, key: string, path: string): JsonValue {
  const value = fields.get(key);
  if (value !== undefined) return value;
  return fail(`${path}: missing required field ${JSON.stringify(key)}`);
}

function nullable<T>(
  fields: Map<string, JsonValue>,
  key: string,
  path: string,
  decode: (value: JsonValue, path: string) => T,
): T | null {
  const value = required(fields, key, path);
  return value.type === "null" ? null : decode(value, `${path}.${key}`);
}

function array<T>(
  value: JsonValue,
  path: string,
  decode: (value: JsonValue, path: string) => T,
): T[] {
  if (value.type !== "array") return fail(`${path}: expected an array`);
  return value.values.map((item, index) => decode(item, `${path}[${index}]`));
}

function boolean(value: JsonValue, path: string): boolean {
  if (value.type === "boolean") return value.value;
  return fail(`${path}: expected a boolean`);
}

function string(value: JsonValue, path: string): string {
  if (value.type === "string") return value.value;
  return fail(`${path}: expected a string`);
}

function integer64(value: JsonValue, path: string): bigint {
  const result = integer(value, path);
  if (result < I64_MIN || result > I64_MAX) {
    return fail(`${path}: integer is outside the signed 64-bit range`);
  }
  return result;
}

function unsignedInteger(value: JsonValue, path: string, maximum: bigint): number {
  const result = integer(value, path);
  if (result < 0n || result > maximum) {
    return fail(`${path}: integer is outside the unsigned range 0...${maximum}`);
  }
  return Number(result);
}

function integer(value: JsonValue, path: string): bigint {
  if (value.type === "integer") return value.value;
  return fail(`${path}: expected an integer JSON token`);
}

function fail(message: string): never {
  throw new Error(`invalid render-tree JSON: ${message}`);
}

class JsonParser {
  private index = 0;

  constructor(private readonly source: string) {}

  parseDocument(): JsonValue {
    this.skipWhitespace();
    const value = this.parseValue();
    this.skipWhitespace();
    if (this.index !== this.source.length) this.error("unexpected trailing content");
    return value;
  }

  private parseValue(): JsonValue {
    const character = this.source[this.index];
    switch (character) {
      case "{":
        return this.parseObject();
      case "[":
        return this.parseArray();
      case '"':
        return { type: "string", value: this.parseString() };
      case "t":
        this.literal("true");
        return { type: "boolean", value: true };
      case "f":
        this.literal("false");
        return { type: "boolean", value: false };
      case "n":
        this.literal("null");
        return { type: "null" };
      default:
        if (character === "-" || isDigit(character)) return this.parseInteger();
        this.error("expected a JSON value");
    }
  }

  private parseObject(): JsonValue {
    this.expect("{");
    this.skipWhitespace();
    const fields = new Map<string, JsonValue>();
    if (this.consume("}")) return { type: "object", fields };

    while (true) {
      if (this.source[this.index] !== '"') this.error("expected an object key");
      const key = this.parseString();
      if (fields.has(key)) this.error(`duplicate object field ${JSON.stringify(key)}`);
      this.skipWhitespace();
      this.expect(":");
      this.skipWhitespace();
      fields.set(key, this.parseValue());
      this.skipWhitespace();
      if (this.consume("}")) return { type: "object", fields };
      this.expect(",");
      this.skipWhitespace();
    }
  }

  private parseArray(): JsonValue {
    this.expect("[");
    this.skipWhitespace();
    const values: JsonValue[] = [];
    if (this.consume("]")) return { type: "array", values };

    while (true) {
      values.push(this.parseValue());
      this.skipWhitespace();
      if (this.consume("]")) return { type: "array", values };
      this.expect(",");
      this.skipWhitespace();
    }
  }

  private parseString(): string {
    this.expect('"');
    let output = "";
    while (this.index < this.source.length) {
      const code = this.source.charCodeAt(this.index);
      if (code === 0x22) {
        this.index += 1;
        return output;
      }
      if (code < 0x20) this.error("unescaped control character in string");
      if (code === 0x5c) {
        this.index += 1;
        output += this.parseEscape();
        continue;
      }
      if (code >= 0xd800 && code <= 0xdbff) {
        const next = this.source.charCodeAt(this.index + 1);
        if (next < 0xdc00 || next > 0xdfff) this.error("unpaired high surrogate in string");
        output += this.source.slice(this.index, this.index + 2);
        this.index += 2;
        continue;
      }
      if (code >= 0xdc00 && code <= 0xdfff) this.error("unpaired low surrogate in string");
      output += this.source[this.index];
      this.index += 1;
    }
    this.error("unterminated string");
  }

  private parseEscape(): string {
    const escape = this.source[this.index];
    this.index += 1;
    switch (escape) {
      case '"':
        return '"';
      case "\\":
        return "\\";
      case "/":
        return "/";
      case "b":
        return "\b";
      case "f":
        return "\f";
      case "n":
        return "\n";
      case "r":
        return "\r";
      case "t":
        return "\t";
      case "u":
        return this.parseUnicodeEscape();
      default:
        this.error("invalid string escape");
    }
  }

  private parseUnicodeEscape(): string {
    const first = this.parseHexCodeUnit();
    if (first >= 0xd800 && first <= 0xdbff) {
      if (this.source.slice(this.index, this.index + 2) !== "\\u") {
        this.error("unpaired high surrogate escape");
      }
      this.index += 2;
      const second = this.parseHexCodeUnit();
      if (second < 0xdc00 || second > 0xdfff) this.error("invalid low surrogate escape");
      return String.fromCodePoint(0x10000 + (first - 0xd800) * 0x400 + (second - 0xdc00));
    }
    if (first >= 0xdc00 && first <= 0xdfff) this.error("unpaired low surrogate escape");
    return String.fromCharCode(first);
  }

  private parseHexCodeUnit(): number {
    const hex = this.source.slice(this.index, this.index + 4);
    if (!/^[0-9a-fA-F]{4}$/.test(hex)) this.error("invalid Unicode escape");
    this.index += 4;
    return Number.parseInt(hex, 16);
  }

  private parseInteger(): JsonValue {
    const start = this.index;
    if (this.consume("-")) {
      if (!isDigit(this.source[this.index])) this.error("expected a digit after minus");
    }

    if (this.consume("0")) {
      if (isDigit(this.source[this.index])) this.error("leading zero in number");
    } else {
      if (!isDigitOneToNine(this.source[this.index])) this.error("expected a digit");
      this.index += 1;
      while (isDigit(this.source[this.index])) this.index += 1;
    }

    if (this.source[this.index] === "." || this.source[this.index] === "e" || this.source[this.index] === "E") {
      this.error("render-tree numbers must use integer JSON tokens");
    }
    return { type: "integer", value: BigInt(this.source.slice(start, this.index)) };
  }

  private literal(value: string): void {
    if (!this.source.startsWith(value, this.index)) this.error(`expected ${value}`);
    this.index += value.length;
  }

  private skipWhitespace(): void {
    while (this.source[this.index] === " " || this.source[this.index] === "\n" || this.source[this.index] === "\r" || this.source[this.index] === "\t") {
      this.index += 1;
    }
  }

  private consume(character: string): boolean {
    if (this.source[this.index] !== character) return false;
    this.index += 1;
    return true;
  }

  private expect(character: string): void {
    if (!this.consume(character)) this.error(`expected ${JSON.stringify(character)}`);
  }

  private error(message: string): never {
    throw new Error(`invalid render-tree JSON at character ${this.index}: ${message}`);
  }
}

function isDigit(character: string | undefined): boolean {
  return character !== undefined && character >= "0" && character <= "9";
}

function isDigitOneToNine(character: string | undefined): boolean {
  return character !== undefined && character >= "1" && character <= "9";
}
