import { expect, test } from "bun:test";

import { decode, Genome } from "../src/genome";
import { Q32_ONE } from "../src/svg/fixed";
import {
  CANVAS_UNITS,
  CLASSIC,
  DispatchError,
  renderSvg,
  renderSvgFor,
} from "../src/svg";

function genome(): Genome {
  return Genome.fromBytes(new Uint8Array(1024));
}

test("supported type and version use the existing renderer", () => {
  const input = genome();
  const expected = renderSvg(decode(input), {
    mood: "happy",
    growth: Q32_ONE,
    canvas: CANVAS_UNITS,
  });

  expect(renderSvgFor(input, CLASSIC, 2, "happy")).toEqual(expected);
});

test("unknown type is refused without rendering", () => {
  const result = catches(() => renderSvgFor(genome(), "creature.wild", 2, "neutral"));

  expect(result).toBeInstanceOf(DispatchError);
  const error = result as DispatchError;
  expect(error.kind).toBe("unsupported-type");
  expect(error.requested).toBe("creature.wild");
});

test("unshipped classic version is refused without rendering", () => {
  const result = catches(() => renderSvgFor(genome(), CLASSIC, 1, "neutral"));

  expect(result).toBeInstanceOf(DispatchError);
  const error = result as DispatchError;
  expect(error.kind).toBe("unsupported-version");
  expect(error.avatarType).toBe(CLASSIC);
  expect(error.requested).toBe(1);
});

test("wrong genome length is refused before rendering", () => {
  const malformed = { asBytes: () => new Uint8Array(1023) } as unknown as Genome;
  const result = catches(() => renderSvgFor(malformed, CLASSIC, 2, "neutral"));

  expect(result).toBeInstanceOf(DispatchError);
  const error = result as DispatchError;
  expect(error.kind).toBe("genome-length");
  expect(error.expected).toBe(1024);
  expect(error.actual).toBe(1023);
});

function catches(action: () => void): unknown {
  try {
    action();
    return null;
  } catch (error) {
    return error;
  }
}
