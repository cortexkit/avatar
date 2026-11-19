import type {
  Effect,
  GradientStop,
  Paint,
  Primitive,
  RenderTree,
  Rgb,
  Segment,
  Shape,
  Stroke,
  Transform,
  TransformOp,
} from "../src/tree";

// This encoder exists only in tests to verify that decoded Rust trees retain
// every wire field. Shipping it would create a second producer for bytes whose
// single source of truth is the canonical Rust encoder.
export function canonicalTreeJsonBytes(tree: RenderTree): Uint8Array {
  const writer = new TreeJsonWriter();
  writer.writeTree(tree);
  return new TextEncoder().encode(writer.finish());
}

type Field = (name: string, writeValue: () => void) => void;

class TreeJsonWriter {
  private output = "";
  /** Canonical JSON of each distinct mask, in table order. */
  private readonly masks: string[] = [];
  /** The table index of each mask, keyed by its canonical JSON. */
  private readonly maskIndices = new Map<string, number>();

  finish(): string {
    return this.output;
  }

  writeTree(tree: RenderTree): void {
    // The mask table comes before the primitives in key order but is complete
    // only once they have been walked, so the primitives are written aside.
    const primitives = this.detached(() =>
      this.array(tree.primitives, (primitive) => this.writePrimitive(primitive)),
    );
    this.fields((field) => {
      field("background", () => this.optional(tree.background, (color) => this.writeRgb(color)));
      field("canvas", () => this.integer(tree.canvas));
      field("masks", () => this.array(this.masks, (entry) => (this.output += entry)));
      field("primitives", () => (this.output += primitives));
    });
  }

  /**
   * Returns the table index of `mask`, adding it when no identical mask is in
   * the table yet. Writing the entry first gives every mask nested inside it
   * an index before this one, as the Rust encoder does.
   */
  private maskRef(mask: readonly Primitive[]): number {
    const entry = this.detached(() => this.array(mask, (primitive) => this.writePrimitive(primitive)));
    const existing = this.maskIndices.get(entry);
    if (existing !== undefined) return existing;
    const index = this.masks.length;
    this.masks.push(entry);
    this.maskIndices.set(entry, index);
    return index;
  }

  /** Runs `write` against an empty output and returns what it wrote. */
  private detached(write: () => void): string {
    const outer = this.output;
    this.output = "";
    write();
    const written = this.output;
    this.output = outer;
    return written;
  }

  private writePrimitive(primitive: Primitive): void {
    this.fields((field) => {
      field("clip", () => this.optional(primitive.clip, (shape) => this.writeShape(shape)));
      field("effects", () => this.array(primitive.effects, (effect) => this.writeEffect(effect)));
      field("label", () => this.string(primitive.label));
      field("opacity", () => this.optional(primitive.opacity, (opacity) => this.integer(opacity)));
      field("paint", () => this.writePaint(primitive.paint));
      field("role", () => this.optional(primitive.role, (role) => this.string(role)));
      field("shape", () => this.writeShape(primitive.shape));
      field("stroke", () => this.optional(primitive.stroke, (stroke) => this.writeStroke(stroke)));
      field("transform", () => this.optional(primitive.transform, (transform) => this.writeTransform(transform)));
    });
  }

  private writeShape(shape: Shape): void {
    switch (shape.kind) {
      case "path":
        this.fields((field) => {
          field("closed", () => this.boolean(shape.closed));
          field("fill_rule", () => this.string(shape.fill_rule));
          field("kind", () => this.string("path"));
          field("segments", () => this.array(shape.segments, (segment) => this.writeSegment(segment)));
        });
        return;
      case "circle":
        this.fields((field) => {
          field("cx", () => this.integer(shape.cx));
          field("cy", () => this.integer(shape.cy));
          field("kind", () => this.string("circle"));
          field("r", () => this.integer(shape.r));
        });
        return;
      case "ellipse":
        this.fields((field) => {
          field("cx", () => this.integer(shape.cx));
          field("cy", () => this.integer(shape.cy));
          field("kind", () => this.string("ellipse"));
          field("rx", () => this.integer(shape.rx));
          field("ry", () => this.integer(shape.ry));
        });
        return;
      case "rect":
        this.fields((field) => {
          field("h", () => this.integer(shape.h));
          field("kind", () => this.string("rect"));
          field("w", () => this.integer(shape.w));
          field("x", () => this.integer(shape.x));
          field("y", () => this.integer(shape.y));
        });
        return;
    }
  }

  private writeSegment(segment: Segment): void {
    switch (segment.kind) {
      case "move_to":
        this.fields((field) => {
          field("kind", () => this.string("move_to"));
          field("x", () => this.integer(segment.x));
          field("y", () => this.integer(segment.y));
        });
        return;
      case "cubic_to":
        this.fields((field) => {
          field("cp1x", () => this.integer(segment.cp1x));
          field("cp1y", () => this.integer(segment.cp1y));
          field("cp2x", () => this.integer(segment.cp2x));
          field("cp2y", () => this.integer(segment.cp2y));
          field("kind", () => this.string("cubic_to"));
          field("x", () => this.integer(segment.x));
          field("y", () => this.integer(segment.y));
        });
        return;
      case "line_to":
        this.fields((field) => {
          field("kind", () => this.string("line_to"));
          field("x", () => this.integer(segment.x));
          field("y", () => this.integer(segment.y));
        });
        return;
    }
  }

  private writePaint(paint: Paint): void {
    switch (paint.kind) {
      case "solid":
        this.fields((field) => {
          field("color", () => this.writeRgb(paint.color));
          field("kind", () => this.string("solid"));
        });
        return;
      case "radial_gradient":
        this.fields((field) => {
          field("kind", () => this.string("radial_gradient"));
          field("stops", () => this.array(paint.stops, (stop) => this.writeGradientStop(stop)));
        });
        return;
      case "none":
        this.fields((field) => field("kind", () => this.string("none")));
        return;
    }
  }

  private writeGradientStop(stop: GradientStop): void {
    this.fields((field) => {
      field("color", () => this.writeRgb(stop.color));
      field("offset_q32", () => this.integer(stop.offset_q32));
    });
  }

  private writeStroke(stroke: Stroke): void {
    this.fields((field) => {
      field("cap", () => this.string(stroke.cap));
      field("color", () => this.writeRgb(stroke.color));
      field("join", () => this.string(stroke.join));
      field("width", () => this.integer(stroke.width));
    });
  }

  private writeEffect(effect: Effect): void {
    switch (effect.kind) {
      case "lit":
        this.fields((field) => {
          field("kind", () => this.string("lit"));
          field("light_y", () => this.integer(effect.light_y));
        });
        return;
      case "masked": {
        const maskRef = this.maskRef(effect.mask);
        this.fields((field) => {
          field("kind", () => this.string("masked"));
          field("mask_ref", () => this.integer(maskRef));
        });
        return;
      }
      case "turbulence":
        this.fields((field) => {
          field("freq_h", () => this.integer(effect.freq_h));
          field("freq_v", () => this.integer(effect.freq_v));
          field("kind", () => this.string("turbulence"));
          field("octaves", () => this.integer(effect.octaves));
          field("scale", () => this.integer(effect.scale));
          field("seed", () => this.integer(effect.seed));
        });
        return;
    }
  }

  private writeTransform(transform: Transform): void {
    this.fields((field) => {
      field("ops", () => this.array(transform.ops, (operation) => this.writeTransformOp(operation)));
    });
  }

  private writeTransformOp(operation: TransformOp): void {
    switch (operation.kind) {
      case "translate":
        this.fields((field) => {
          field("kind", () => this.string("translate"));
          field("tx", () => this.integer(operation.tx));
          field("ty", () => this.integer(operation.ty));
        });
        return;
      case "rotate":
        this.fields((field) => {
          field("angle", () => this.integer(operation.angle));
          field("cx", () => this.integer(operation.cx));
          field("cy", () => this.integer(operation.cy));
          field("kind", () => this.string("rotate"));
        });
        return;
      case "scale":
        this.fields((field) => {
          field("kind", () => this.string("scale"));
          field("sx", () => this.integer(operation.sx));
          field("sy", () => this.integer(operation.sy));
        });
        return;
    }
  }

  private writeRgb(color: Rgb): void {
    this.fields((field) => {
      field("b", () => this.integer(color.b));
      field("g", () => this.integer(color.g));
      field("r", () => this.integer(color.r));
    });
  }

  private fields(writeFields: (field: Field) => void): void {
    this.output += "{";
    let first = true;
    writeFields((name, writeValue) => {
      if (first) first = false;
      else this.output += ",";
      this.string(name);
      this.output += ":";
      writeValue();
    });
    this.output += "}";
  }

  private array<T>(values: readonly T[], writeValue: (value: T) => void): void {
    this.output += "[";
    for (let index = 0; index < values.length; index += 1) {
      if (index !== 0) this.output += ",";
      writeValue(values[index]);
    }
    this.output += "]";
  }

  private optional<T>(value: T | null, writeValue: (value: T) => void): void {
    if (value === null) this.output += "null";
    else writeValue(value);
  }

  private integer(value: bigint | number): void {
    this.output += value.toString();
  }

  private boolean(value: boolean): void {
    this.output += value ? "true" : "false";
  }

  private string(value: string): void {
    this.output += '"';
    for (const character of value) {
      switch (character) {
        case '"':
          this.output += '\\"';
          break;
        case "\\":
          this.output += "\\\\";
          break;
        case "\b":
          this.output += "\\b";
          break;
        case "\f":
          this.output += "\\f";
          break;
        case "\n":
          this.output += "\\n";
          break;
        case "\r":
          this.output += "\\r";
          break;
        case "\t":
          this.output += "\\t";
          break;
        default: {
          const codePoint = character.codePointAt(0);
          if (codePoint !== undefined && codePoint <= 0x1f) {
            this.output += `\\u${codePoint.toString(16).padStart(4, "0")}`;
          } else {
            this.output += character;
          }
        }
      }
    }
    this.output += '"';
  }
}
