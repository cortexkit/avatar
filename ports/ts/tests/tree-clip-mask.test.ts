import { expect, test } from "bun:test";

import { serializeTree } from "../src/tree-serialize";
import type { Primitive, Shape } from "../src/tree";

const white: Primitive = {
  shape: { kind: "rect", x: 0n, y: 0n, w: 1000n, h: 1000n },
  transform: null,
  paint: { kind: "solid", color: { r: 255, g: 255, b: 255 } },
  stroke: null,
  opacity: null,
  clip: null,
  effects: [],
  label: "",
  role: null,
};

test("transforms structural clip and shape together", () => {
  const svg = serializeTree({ canvas: 10, background: null, primitives: [{
    ...white,
    shape: { kind: "circle", cx: 100n, cy: 100n, r: 150n },
    clip: { kind: "rect", x: 0n, y: 0n, w: 200n, h: 200n },
    transform: { ops: [{ kind: "translate", tx: 500n, ty: 600n }] },
  }] });
  expect(svg).toContain(
    '<g transform="translate(5 6)"><g clip-path="url(#clip-primitive-0)"><circle cx="1" cy="1" r="1.5" fill="#ffffff"/></g></g>',
  );
});

test("preserves even-odd inside both clip definitions", () => {
  const path: Shape = {
    kind: "path", segments: [{ kind: "move_to", x: 0n, y: 0n }],
    closed: true, fill_rule: "even_odd",
  };
  const svg = serializeTree({ canvas: 10, background: null, primitives: [{
    ...white, shape: path, clip: path,
    effects: [{ kind: "masked", mask: [{ ...white, shape: path }] }],
  }] });
  expect(svg.match(/clip-rule="evenodd"/g) ?? []).toHaveLength(2);
  expect(svg.match(/fill-rule="evenodd"/g) ?? []).toHaveLength(1);
});

for (const [value, fill] of [[0, "#000000"], [239, "#efefef"]] as const) {
  test(`keeps solid ${fill} mask as luminance`, () => {
    const svg = serializeTree({ canvas: 10, background: null, primitives: [{
      ...white,
      effects: [{ kind: "masked", mask: [{
        ...white, paint: { kind: "solid", color: { r: value, g: value, b: value } },
      }] }],
    }] });
    expect(svg).toContain('mask-type="luminance"');
    expect(svg).toContain(`fill="${fill}"`);
    expect(svg).not.toContain("<clipPath");
  });
}
