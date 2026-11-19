import { formatMilli, formatQ32Rounded } from "./tree-format";
import type { Effect, Paint, Primitive, RenderTree, Shape, TransformOp } from "./tree";

/** Serializes every primitive in painter order without interpreting its label. */
export function serializeTree(tree: RenderTree): string {
  const definitions: string[] = [];
  let content = "";

  let index = 0;
  while (index < tree.primitives.length) {
    const effects = tree.primitives[index].effects;
    let end = index + 1;
    while (
      end < tree.primitives.length &&
      valuesEqual(tree.primitives[end].effects, effects)
    ) {
      end += 1;
    }

    let members = "";
    for (let primitiveIndex = index; primitiveIndex < end; primitiveIndex += 1) {
      members += serializePrimitive(
        tree.primitives[primitiveIndex],
        `primitive-${primitiveIndex}`,
        definitions,
        false,
        tree.canvas,
      );
      members += "\n";
    }
    content += wrapEffects(members, effects, `group-${index}`, definitions, tree.canvas);
    index = end;
  }

  let output = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${tree.canvas} ${tree.canvas}">\n`;
  if (definitions.length !== 0) {
    output += `<defs>\n${definitions.join("")}</defs>\n`;
  }
  if (tree.background !== null) {
    output += `<rect width="${tree.canvas}" height="${tree.canvas}" fill="${hex(tree.background.r, tree.background.g, tree.background.b)}"/>\n`;
  }
  output += content;
  output += "</svg>\n";
  return output;
}

function serializePrimitive(
  primitive: Primitive,
  key: string,
  definitions: string[],
  includeEffects: boolean,
  maskExtent: number,
): string {
  const paint = paintValue(primitive.paint, `gradient-${key}`, definitions);
  let attributes = `fill="${paint}"`;
  if (primitive.stroke !== null) {
    attributes +=
      ` stroke="${hex(primitive.stroke.color.r, primitive.stroke.color.g, primitive.stroke.color.b)}"` +
      ` stroke-width="${formatMilli(primitive.stroke.width)}"` +
      ` stroke-linecap="${primitive.stroke.cap}"` +
      ` stroke-linejoin="${primitive.stroke.join}"`;
  }
  if (primitive.opacity !== null) {
    attributes += ` opacity="${formatQ32Rounded(primitive.opacity)}"`;
  }
  if (primitive.label.length !== 0) {
    attributes += ` class="${xmlEscape(primitive.label)}"`;
  }
  if (primitive.transform !== null) {
    attributes += ` transform="${transformValue(primitive.transform.ops)}"`;
  }

  let element = shapeElement(primitive.shape, attributes);
  if (primitive.clip !== null) {
    const clipId = `clip-${key}`;
    definitions.push(
      `<clipPath id="${clipId}" clipPathUnits="userSpaceOnUse">${shapeElement(primitive.clip, "", true)}</clipPath>\n`,
    );
    element = `<g clip-path="url(#${clipId})">${element}</g>`;
  }

  return includeEffects
    ? wrapEffects(element, primitive.effects, key, definitions, maskExtent)
    : element;
}

function wrapEffects(
  initialContent: string,
  effects: Effect[],
  key: string,
  definitions: string[],
  maskExtent: number,
): string {
  let content = initialContent;
  for (let effectIndex = effects.length - 1; effectIndex >= 0; effectIndex -= 1) {
    const effect = effects[effectIndex];
    if (effect.kind !== "masked") continue;

    const maskId = `mask-${key}-${effectIndex}`;
    if (isClipMask(effect.mask)) {
      definitions.push(
        `<clipPath id="${maskId}" clipPathUnits="userSpaceOnUse">${clipShape(effect.mask[0])}</clipPath>\n`,
      );
      content = `<g clip-path="url(#${maskId})">${content}</g>`;
      continue;
    }

    let maskContent = "";
    for (let maskIndex = 0; maskIndex < effect.mask.length; maskIndex += 1) {
      maskContent += serializePrimitive(
        effect.mask[maskIndex],
        `${key}-mask-${effectIndex}-${maskIndex}`,
        definitions,
        true,
        maskExtent,
      );
    }
    definitions.push(
      `<mask id="${maskId}" x="${-maskExtent}" y="${-maskExtent}" width="${maskExtent * 3}" height="${maskExtent * 3}" maskUnits="userSpaceOnUse" mask-type="luminance" maskContentUnits="userSpaceOnUse">${maskContent}</mask>\n`,
    );
    content = `<g mask="url(#${maskId})">${content}</g>`;
  }
  return content;
}

function isClipMask(mask: Primitive[]): boolean {
  if (mask.length !== 1) return false;
  const primitive = mask[0];
  return (
    primitive.paint.kind === "solid" &&
    primitive.paint.color.r === 255 &&
    primitive.paint.color.g === 255 &&
    primitive.paint.color.b === 255 &&
    primitive.opacity === null &&
    primitive.stroke === null &&
    primitive.clip === null &&
    primitive.effects.length === 0
  );
}

function clipShape(primitive: Primitive): string {
  const attributes =
    primitive.transform === null ? "" : `transform="${transformValue(primitive.transform.ops)}"`;
  return shapeElement(primitive.shape, attributes, true);
}

function paintValue(paint: Paint, gradientId: string, definitions: string[]): string {
  switch (paint.kind) {
    case "solid":
      return hex(paint.color.r, paint.color.g, paint.color.b);
    case "radial_gradient": {
      let definition = `<radialGradient id="${gradientId}">`;
      for (const stop of paint.stops) {
        definition += `<stop offset="${formatQ32Rounded(stop.offset_q32)}" stop-color="${hex(stop.color.r, stop.color.g, stop.color.b)}"/>`;
      }
      definitions.push(`${definition}</radialGradient>\n`);
      return `url(#${gradientId})`;
    }
    case "none":
      return "none";
  }
}

function shapeElement(shape: Shape, attributes: string, inClip = false): string {
  const suffix = attributes.length === 0 ? "" : ` ${attributes}`;
  switch (shape.kind) {
    case "path": {
      const rule = shape.fill_rule === "non_zero" ? 'fill-rule="nonzero"'
        : inClip ? 'clip-rule="evenodd"' : 'fill-rule="evenodd"';
      return `<path d="${pathData(shape.segments, shape.closed)}" ${rule}${suffix}/>`;
    }
    case "circle":
      return `<circle cx="${formatMilli(shape.cx)}" cy="${formatMilli(shape.cy)}" r="${formatMilli(shape.r)}"${suffix}/>`;
    case "ellipse":
      return `<ellipse cx="${formatMilli(shape.cx)}" cy="${formatMilli(shape.cy)}" rx="${formatMilli(shape.rx)}" ry="${formatMilli(shape.ry)}"${suffix}/>`;
    case "rect":
      return `<rect x="${formatMilli(shape.x)}" y="${formatMilli(shape.y)}" width="${formatMilli(shape.w)}" height="${formatMilli(shape.h)}"${suffix}/>`;
  }
}

function pathData(segments: Extract<Shape, { kind: "path" }>["segments"], closed: boolean): string {
  let output = "";
  for (const segment of segments) {
    switch (segment.kind) {
      case "move_to":
        output += `M${formatMilli(segment.x)} ${formatMilli(segment.y)}`;
        break;
      case "line_to":
        output += ` L${formatMilli(segment.x)} ${formatMilli(segment.y)}`;
        break;
      case "cubic_to":
        output +=
          ` C${formatMilli(segment.cp1x)} ${formatMilli(segment.cp1y)},` +
          `${formatMilli(segment.cp2x)} ${formatMilli(segment.cp2y)},` +
          `${formatMilli(segment.x)} ${formatMilli(segment.y)}`;
        break;
    }
  }
  return closed ? `${output} Z` : output;
}

function transformValue(operations: TransformOp[]): string {
  return operations
    .map((operation) => {
      switch (operation.kind) {
        case "translate":
          return `translate(${formatMilli(operation.tx)} ${formatMilli(operation.ty)})`;
        case "rotate":
          return `rotate(${bamDegrees(operation.angle)} ${formatMilli(operation.cx)} ${formatMilli(operation.cy)})`;
        case "scale":
          return `scale(${formatQ32Rounded(operation.sx)} ${formatQ32Rounded(operation.sy)})`;
      }
    })
    .join(" ");
}

function bamDegrees(angle: number): string {
  const scale = 1_000_000n;
  const scaled = (BigInt(angle) * 360n * scale + 32_768n) / 65_536n;
  const whole = scaled / scale;
  const fraction = scaled % scale;
  if (fraction === 0n) return whole.toString();
  return `${whole}.${fraction.toString().padStart(6, "0").replace(/0+$/, "")}`;
}

function hex(r: number, g: number, b: number): string {
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

function xmlEscape(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function valuesEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (left === null || right === null || typeof left !== "object" || typeof right !== "object") {
    return false;
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => valuesEqual(value, right[index]))
    );
  }

  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const keys = Object.keys(leftRecord);
  return (
    keys.length === Object.keys(rightRecord).length &&
    keys.every(
      (key) =>
        Object.prototype.hasOwnProperty.call(rightRecord, key) &&
        valuesEqual(leftRecord[key], rightRecord[key]),
    )
  );
}
