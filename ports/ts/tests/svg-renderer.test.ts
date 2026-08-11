// Unit tests for the SVG renderer port. Each describe block mirrors the
// #[cfg(test)] module of the corresponding Rust module in
// crates/avatar-svg/src/, so the port and the reference keep the same
// pinned arithmetic under test. Expected values are copied from the Rust
// tests, not recomputed.

import { describe, expect, test } from "bun:test";

import type {
  Body,
  BodyPoint,
  Circler,
  Cut,
  Mouth,
  Palette,
  Pattern,
  Phenotype,
  Rgb,
  Voronoi,
} from "../src/phenotype";
import { divRoundHalfAway } from "../src/tables";
import { VORONOI_PATTERNS } from "../src/generated/voronoi";

import { hexColor } from "../src/svg/colors";import {
  bamDegreesQ32,
  CANVAS_MILLI,
  CENTER,
  divI64,
  lerpQ32Milli,
  oppositeBam,
  Q32_ONE,
  q32Mul,
  q32Ratio,
} from "../src/svg/fixed";
import { formatMilli, formatQ32Decimal } from "../src/svg/format";
import {
  adjustPoint,
  bezierSample,
  bodyGeometry,
  buildSpline,
  flatten,
  mirrorRing,
  minWidthY,
  pathString,
  rescale,
  type RingPoint,
  type Spline,
} from "../src/svg/geometry";
import { cubicPoint, sampleCount, type Cubic } from "../src/svg/lashes";
import { MOOD_DELTAS_ZERO, moodDeltas, scaleMoodDeltas } from "../src/svg/mood";
import { renderMouth, shapeFor } from "../src/svg/mouth";
import {
  circlerPath,
  cuttingBezierPath,
  maskContent,
  voronoiPatternsHash,
  voronoiPlacement,
} from "../src/svg/pattern";
import {
  CANVAS_UNITS,
  lightZ,
  placementFor,
  renderBodySvg,
  renderSvg,
  RenderError,
  strokeWidthFor,
  type Mood,
  type RenderParams,
} from "../src/svg/svg";
import { eyeLineAnchor, findRect, MouthRect } from "../src/svg/face";
import type { BodyGeometry } from "../src/svg/geometry";

const ALL_MOODS: readonly Mood[] = [
  "neutral",
  "working",
  "happy",
  "waiting",
  "blocked",
  "sad",
  "sleepy",
];

describe("fixed (fixed.rs tests)", () => {
  test("q32Mul rounds half away from zero", () => {
    expect(q32Mul(3n, Q32_ONE / 2n)).toBe(2n);
    expect(q32Mul(-3n, Q32_ONE / 2n)).toBe(-2n);
    expect(q32Mul(100n, Q32_ONE)).toBe(100n);
  });

  test("lerp hits endpoints exactly", () => {
    expect(lerpQ32Milli(10n, 20n, 0n)).toBe(10n);
    expect(lerpQ32Milli(10n, 20n, Q32_ONE)).toBe(20n);
    expect(lerpQ32Milli(0n, 100n, Q32_ONE / 2n)).toBe(50n);
  });

  test("ratio of full canvas is one", () => {
    expect(q32Ratio(CANVAS_MILLI)).toBe(Q32_ONE);
    expect(q32Ratio(CANVAS_MILLI / 2n)).toBe(Q32_ONE / 2n);
  });

  test("divI64 applies the sign of the divisor", () => {
    // The eye highlights divide by negative divisors on purpose.
    expect(divI64(100n, -5n)).toBe(-20n);
    expect(divI64(-100n, -5n)).toBe(20n);
    expect(divI64(7n, 2n)).toBe(4n);
    expect(divI64(-7n, 2n)).toBe(-4n);
    expect(() => divI64(1n, 0n)).toThrow(RangeError);
  });

  test("bam degrees interpret wrapped negatives through i16", () => {
    expect(bamDegreesQ32(16_384n)).toBe(90n * Q32_ONE);
    // 63_898 wraps to −1_638 BAM = −1638·360/65536 degrees, exact in Q32
    // because 2^32/65536 = 65536 folds into the product.
    expect(bamDegreesQ32(63_898n)).toBe(-1_638n * 360n * 65_536n);
    expect(oppositeBam(63_898n)).toBe(1_638n);
    expect(oppositeBam(0n)).toBe(0n);
  });
});

describe("format (format.rs tests, verbatim cases)", () => {
  test("milli formatter covers the pinned cases", () => {
    expect(formatMilli(0n)).toBe("0");
    expect(formatMilli(100n)).toBe("1");
    expect(formatMilli(150n)).toBe("1.5");
    expect(formatMilli(105n)).toBe("1.05");
    expect(formatMilli(-150n)).toBe("-1.5");
    expect(formatMilli(99n)).toBe("0.99");
    expect(formatMilli(-99n)).toBe("-0.99");
    expect(formatMilli(1_000_000n)).toBe("10000");
  });

  test("milli formatter never emits negative zero", () => {
    for (let value = -99n; value <= 99n; value += 1n) {
      expect(formatMilli(value)).not.toBe("-0");
    }
  });

  test("milli formatter is exact for every remainder class", () => {
    // 12_345 = 123.45 units: both fractional digits survive.
    expect(formatMilli(12_345n)).toBe("123.45");
    // 12_340 = 123.40 units: trailing zero stripped.
    expect(formatMilli(12_340n)).toBe("123.4");
    // 12_300 = 123 units: no fractional part.
    expect(formatMilli(12_300n)).toBe("123");
    expect(formatMilli(-12_345n)).toBe("-123.45");
  });

  test("q32 formatter emits integer and fraction", () => {
    expect(formatQ32Decimal(0n)).toBe("0");
    expect(formatQ32Decimal(Q32_ONE)).toBe("1");
    expect(formatQ32Decimal(Q32_ONE / 2n)).toBe("0.5");
    expect(formatQ32Decimal(Q32_ONE / 4n)).toBe("0.25");
    expect(formatQ32Decimal(-Q32_ONE / 2n)).toBe("-0.5");
    expect(formatQ32Decimal(2n * Q32_ONE + Q32_ONE / 2n)).toBe("2.5");
  });

  test("q32 formatter truncates to six digits", () => {
    // 1/3 in Q32.32 (floor): 0.333333333... truncates to six digits.
    const third = (1n << 32n) / 3n;
    expect(formatQ32Decimal(third)).toBe("0.333333");
  });

  test("q32 formatter strips trailing zeros", () => {
    // 0.1 in Q32.32 (half-away): 0.100000000... emits one digit.
    const tenth = ((1n << 32n) + 5n) / 10n;
    expect(formatQ32Decimal(tenth)).toBe("0.1");
  });
});

describe("geometry (geometry.rs tests)", () => {
  const point = (x: bigint, y: bigint, tension: bigint): BodyPoint => ({ x, y, tension });

  function bodyWith(points: BodyPoint[]): Body {
    const padding: BodyPoint = { x: 0n, y: 0n, tension: 0n };
    return {
      point_count: BigInt(points.length),
      size: CANVAS_MILLI,
      points: [...points, ...Array(16 - points.length).fill(padding)],
    };
  }

  test("mirror of three points is a six point ring", () => {
    // max_y = 400_000 → vertical shift = 500_000 − 200_000 = 300_000.
    const ring = mirrorRing(
      bodyWith([
        point(100_000n, 200_000n, Q32_ONE),
        point(300_000n, 400_000n, 2n * Q32_ONE),
        point(500_000n, 100_000n, Q32_ONE / 2n),
      ]),
    );
    expect(ring).toEqual([
      // Left side, i = 0..3: x = center − x/2.
      { x: 450_000n, y: 500_000n, tension: Q32_ONE },
      { x: 350_000n, y: 700_000n, tension: 2n * Q32_ONE },
      { x: 250_000n, y: 400_000n, tension: Q32_ONE / 2n },
      // Right side mirrors points[2], points[1], then points[0]:
      // every half-side point appears once per side.
      { x: 750_000n, y: 400_000n, tension: Q32_ONE / 2n },
      { x: 650_000n, y: 700_000n, tension: 2n * Q32_ONE },
      { x: 550_000n, y: 500_000n, tension: Q32_ONE },
    ]);
  });

  test("mirror halving rounds half away from zero", () => {
    const ring = mirrorRing(
      bodyWith([point(100_001n, 0n, Q32_ONE), point(-100_001n, 10n, Q32_ONE)]),
    );
    // +100_001/2 = 50_000.5 → 50_001; −100_001/2 = −50_000.5 → −50_001.
    expect(ring[0].x).toBe(500_000n - 50_001n);
    expect(ring[1].x).toBe(500_000n + 50_001n);
  });

  test("spline closes the ring", () => {
    const ring = mirrorRing(
      bodyWith([
        point(100_000n, 200_000n, Q32_ONE),
        point(300_000n, 400_000n, Q32_ONE),
        point(500_000n, 100_000n, Q32_ONE),
      ]),
    );
    const spline = buildSpline(ring);
    expect(spline.curves.length).toBe(ring.length);
    // The final segment ends where the path started.
    const last = spline.curves[spline.curves.length - 1];
    expect([last.x2, last.y2]).toEqual([spline.startX, spline.startY]);
    // The first segment starts at the ring's last point (cursor len−2).
    expect([spline.startX, spline.startY]).toEqual([ring[ring.length - 1].x, ring[ring.length - 1].y]);
  });

  test("tension walk skips the seam segment", () => {
    // Five ring points on a line with distinct tensions. Expected control
    // points are hand-computed with the pinned arithmetic (all products
    // divide evenly, so no rounding is hidden in the literals):
    //   offset = delta · tension / (6 · 2^32)
    // Walk: segments take ring tensions [t0, t1, t2, t2, t3] — the
    // increment is skipped after segment 2 (len/2 = 2), so segment 3
    // reuses t2 and t4 is never read. Without the skip, segment 3's cp1x
    // would be 180 and segment 4's cp1x 190 instead of 140 and 240.
    const one = Q32_ONE;
    const ring: RingPoint[] = [
      { x: 0n, y: 0n, tension: one },
      { x: 60n, y: 0n, tension: 3n * one },
      { x: 120n, y: 0n, tension: one },
      { x: 180n, y: 0n, tension: 3n * one },
      { x: 240n, y: 0n, tension: one / 2n },
    ];
    const spline = buildSpline(ring);
    expect([spline.startX, spline.startY]).toEqual([240n, 0n]);
    const expected: Array<[bigint, bigint, bigint, bigint, bigint, bigint]> = [
      // seg0: window P3 P4 P0 P1, tension t0 = 1.
      [210n, 0n, 30n, 0n, 0n, 0n],
      // seg1: window P4 P0 P1 P2, tension t1 = 3.
      [-90n, 0n, 0n, 0n, 60n, 0n],
      // seg2: window P0 P1 P2 P3, tension t2 = 1.
      [80n, 0n, 100n, 0n, 120n, 0n],
      // seg3: window P1 P2 P3 P4, tension t2 again (the skip).
      [140n, 0n, 160n, 0n, 180n, 0n],
      // seg4: window P2 P3 P4 P0, tension t3 = 3.
      [240n, 0n, 330n, 0n, 240n, 0n],
    ];
    spline.curves.forEach((curve, index) => {
      const [cp1x, cp1y, cp2x, cp2y, x2, y2] = expected[index];
      expect([curve.cp1x, curve.cp1y, curve.cp2x, curve.cp2y, curve.x2, curve.y2]).toEqual([
        cp1x,
        cp1y,
        cp2x,
        cp2y,
        x2,
        y2,
      ]);
    });
  });

  test("flatten samples every segment and hits endpoints exactly", () => {
    const ring = mirrorRing(
      bodyWith([
        point(100_000n, 200_000n, Q32_ONE),
        point(300_000n, 400_000n, Q32_ONE),
        point(500_000n, 100_000n, Q32_ONE),
      ]),
    );
    const spline = buildSpline(ring);
    const bounds = flatten(spline);
    expect(bounds.samples.length).toBe(spline.curves.length * 21);
    expect(bounds.samples[0]).toEqual([spline.startX, spline.startY]);
    spline.curves.forEach((curve, index) => {
      expect(bounds.samples[index * 21 + 20]).toEqual([curve.x2, curve.y2]);
    });
    expect(bounds.minX <= bounds.maxX).toBe(true);
    expect(bounds.minY <= bounds.maxY).toBe(true);
  });

  test("flatten tracks extremes first wins on ties", () => {
    // A straight horizontal Bézier: every sample has the same y, so the
    // first sample must own both y extremes (strict comparisons).
    const spline: Spline = {
      startX: 0n,
      startY: 100n,
      curves: [
        {
          cp1x: 100n,
          cp1y: 100n,
          cp2x: 200n,
          cp2y: 100n,
          x2: 300n,
          y2: 100n,
        },
      ],
    };
    const bounds = flatten(spline);
    expect(bounds.minY).toBe(100n);
    expect(bounds.maxY).toBe(100n);
    expect(bounds.minX).toBe(0n);
    expect(bounds.maxX).toBe(300n);
    expect(bounds.maxWidthY).toBe(100n);
  });

  test("bezier endpoints are exact", () => {
    expect(bezierSample(10n, 20n, 30n, 40n, 0n)).toBe(10n);
    expect(bezierSample(10n, 20n, 30n, 40n, 20n)).toBe(40n);
    expect(bezierSample(-7n, 3n, 5n, 11n, 0n)).toBe(-7n);
    expect(bezierSample(-7n, 3n, 5n, 11n, 20n)).toBe(11n);
  });

  test("adjust point pulls toward center by ratio", () => {
    const ratio = 2n * Q32_ONE;
    expect(adjustPoint(750_000n, ratio)).toBe(625_000n);
    expect(adjustPoint(250_000n, ratio)).toBe(375_000n);
    expect(adjustPoint(-100_000n, ratio)).toBe(200_000n);
    expect(adjustPoint(CENTER, ratio)).toBe(CENTER);
  });

  test("rescale leaves fitting bodies untouched", () => {
    const ring = mirrorRing(
      bodyWith([
        point(100_000n, 200_000n, Q32_ONE),
        point(300_000n, 400_000n, Q32_ONE),
        point(500_000n, 100_000n, Q32_ONE),
      ]),
    );
    const spline = buildSpline(ring);
    const bounds = flatten(spline);
    const before = structuredClone(spline);
    const width = rescale(spline, bounds);
    expect(spline).toEqual(before);
    expect(width).toBe(bounds.maxX - bounds.minX);
  });

  test("rescale shrinks an overflowing body", () => {
    // Points far outside the canvas force final_ratio > 1.
    const ring = mirrorRing(
      bodyWith([
        point(2_000_000n, 0n, Q32_ONE),
        point(2_400_000n, 3_000_000n, Q32_ONE),
        point(1_800_000n, 100_000n, Q32_ONE),
      ]),
    );
    const spline = buildSpline(ring);
    const bounds = flatten(spline);
    const width = rescale(spline, bounds);
    // After rescale every sample must sit inside the canvas again.
    for (const [x, y] of bounds.samples) {
      expect(x >= -1n && x <= CANVAS_MILLI + 1n).toBe(true);
      expect(y >= -1n && y <= CANVAS_MILLI + 1n).toBe(true);
    }
    expect(width <= CANVAS_MILLI).toBe(true);
  });

  test("min width y picks the most inset band midpoint", () => {
    // Every band 1..=8 carries samples so the empty-band quirk stays out
    // of play; band 3 has the largest left inset (its min x is the
    // greatest), so its midpoint must win.
    const samples: Array<[bigint, bigint]> = [
      [50n, 150n],
      [40n, 250n],
      [100n, 350n],
      [30n, 450n],
      [35n, 550n],
      [45n, 650n],
      [55n, 750n],
      [60n, 850n],
    ];
    // Band 3 midpoint: top + 3·h/10 + h/20 = 300 + 50 = 350.
    expect(minWidthY(samples, 0n, 1_000n)).toBe(350n);
  });

  test("min width y empty first band wins", () => {
    // Reference quirk: an empty band keeps the i64-max sentinel as its
    // inset and outranks every real sample, so the first empty band's
    // midpoint is returned.
    const samples: Array<[bigint, bigint]> = [[100n, 750n]];
    expect(minWidthY(samples, 0n, 1_000n)).toBe(150n);
  });

  test("path string layout matches the pinned shape", () => {
    const spline: Spline = {
      startX: 500_000n,
      startY: 250_000n,
      curves: [
        {
          cp1x: 500_150n,
          cp1y: 250_000n,
          cp2x: 500_105n,
          cp2y: 250_100n,
          x2: 500_000n,
          y2: 250_200n,
        },
      ],
    };
    expect(pathString(spline)).toBe("M5000,2500C5001.5 2500, 5001.05 2501, 5000 2502");
  });
});

describe("pattern (pattern.rs tests)", () => {
  const circler = (
    pointReduce: bigint,
    size: bigint,
    yOffsetDiv: bigint,
    tension: bigint,
    pullMin: bigint,
  ): Circler => ({
    point_reduce: pointReduce,
    size,
    y_offset_div: yOffsetDiv,
    tension,
    pull_min: pullMin,
  });

  const patternWith = (klass: bigint): Pattern => ({
    class: klass,
    circler: circler(0n, 0n, 0n, 0n, 0n),
    voronoi: { index: 0n, rotation: 0n, scale: 0n },
    cut: { spread: 0n, y_adjust_div: 0n },
  });

  test("class zero has no mask content", () => {
    expect(maskContent(patternWith(0n), 500_000n, 500_000n, 250_000n)).toBeNull();
  });

  test("circler ring has the expected point count", () => {
    // point_reduce byte 0 → EASE_IN_QUINT[0] = 0 → numPoints = 16 → 32
    // ring points → 32 Bézier segments in the path.
    const path = circlerPath(circler(0n, Q32_ONE / 2n, Q32_ONE / 2n, 0n, Q32_ONE), true, 400_000n, 300_000n);
    expect(path.split("C").length - 1).toBe(32);
    expect(path.startsWith("M")).toBe(true);
  });

  test("circler point reduce shrinks the ring", () => {
    // point_reduce byte 255 → EASE_IN_QUINT[255] = 1.0 → reduction 8 →
    // numPoints 8 → 16 segments.
    const path = circlerPath(circler(255n, Q32_ONE / 2n, Q32_ONE / 2n, 0n, Q32_ONE), false, 400_000n, 300_000n);
    expect(path.split("C").length - 1).toBe(16);
  });

  test("cutting bezier layout is pinned", () => {
    const cut: Cut = {
      spread: 0n, // lerp(0.5, 10, 0) = 0.5 → lr = canvas/2
      y_adjust_div: 0n, // lerp(2, 100, 0) = 2 → yAdjust = h/2
    };
    const path = cuttingBezierPath(cut, 200_000n, 100_000n);
    // lr = 500_000, yAdjust = 100_000: yBottom = 400_000, yTop = 200_000,
    // yMid = 700_000, cxLeft = 0, cxRight = 1_000_000.
    expect(path).toBe(
      "M 0, 4000 C 0 4000, 5000 7000, 5000 2000C 5000 7000, 10000 4000, 10000 4000Z",
    );
  });

  test("voronoi index maps into the vendored table", () => {
    const cases: Array<[bigint, number]> = [
      [0n, 0],
      [64n, 1],
      [128n, 2],
      [191n, 3],
      [255n, 4],
    ];
    for (const [byte, expected] of cases) {
      const index = Number(divRoundHalfAway(byte * BigInt(VORONOI_PATTERNS.length - 1), 255n));
      expect(index).toBe(expected);
    }
  });

  test("voronoi placement scales to cover the body", () => {
    const voronoi: Voronoi = {
      index: 0n,
      rotation: 16_384n, // quarter turn → 90°
      scale: 0n, // lerp(0.1, 2.5, 0) = 0.1, below the body ratio
    };
    const [path, , , scale, degrees, rotateCenter] = voronoiPlacement(voronoi, 900_000n, 400_000n);
    expect(path).toBe(VORONOI_PATTERNS[0]);
    expect(degrees).toBe(90n);
    // Scale is at least the body-width ratio times canvas/255.
    const strokeUnit = divRoundHalfAway(CANVAS_MILLI, 255n);
    const minScale = q32Mul(q32Ratio(900_000n), strokeUnit);
    expect(scale >= minScale).toBe(true);
    expect(rotateCenter > 0n).toBe(true);
  });

  test("mask content wraps the path in three groups", () => {
    const content = maskContent(patternWith(4n), 500_000n, 200_000n, 100_000n);
    expect(content).not.toBeNull();
    expect(content!.split("<g").length - 1).toBe(3);
    expect(content!.split("</g>").length - 1).toBe(3);
    expect(content!.includes('fill="#efefef"')).toBe(true);
    expect(content!.includes("rotate(0 5000 5000)")).toBe(true);
  });

  test("voronoi table hash matches the Rust pin", () => {
    // Pin source: crates/avatar-svg/src/lib.rs, voronoi_table_hash_is_pinned.
    expect(voronoiPatternsHash()).toBe(
      "0f080cce4b31019700389825292df78b9fe921e8d410acb2ed763952e899a150",
    );
  });
});

describe("mood (mood.rs tests)", () => {
  test("neutral is the literal zero row", () => {
    expect(moodDeltas("neutral")).toEqual(MOOD_DELTAS_ZERO);
  });

  test("pinned rows keep the intended lid ordering", () => {
    const blocked = moodDeltas("blocked");
    const sad = moodDeltas("sad");
    const sleepy = moodDeltas("sleepy");
    const waiting = moodDeltas("waiting");
    expect(blocked.upperLidClose > sad.upperLidClose).toBe(true);
    expect(sleepy.upperLidClose > blocked.upperLidClose).toBe(true);
    expect(waiting.upperLidClose < 0n).toBe(true);
    expect(waiting.lowerLidClose < 0n).toBe(true);
  });

  test("scaling handles wrapped negative angles", () => {
    const response = {
      lid_reactivity: Q32_ONE,
      brow_reactivity: Q32_ONE / 2n,
      eye_rotation_reactivity: Q32_ONE,
      mouth_reactivity: Q32_ONE,
    };
    const scaled = scaleMoodDeltas(moodDeltas("sad"), response);
    expect(scaled.eyeRotation).toBe(63_898n);
    // 63_716 wraps to −1_820; half of that is −910, re-wrapped.
    const signedBrow = scaled.browRotation >= 0x8000n ? scaled.browRotation - 0x10000n : scaled.browRotation;
    expect(signedBrow).toBe(-910n);
  });
});

describe("lashes (lashes.rs tests)", () => {
  test("count and removal match the reference loop", () => {
    expect(sampleCount(3n, 0n)).toBe(4);
    expect(sampleCount(16n, 2n)).toBe(11);
    expect(sampleCount(8n, 3n)).toBe(6);
    expect(sampleCount(2n, 3n)).toBe(0);
    expect(sampleCount(0n, 0n)).toBe(0);
  });

  test("cubic sampling hits both endpoints", () => {
    const curve: Cubic = {
      x0: 10n,
      y0: 20n,
      cp1x: 30n,
      cp1y: 0n,
      cp2x: 70n,
      cp2y: 0n,
      x3: 90n,
      y3: 20n,
    };
    expect(cubicPoint(curve, 0n, 10n)).toEqual([10n, 20n]);
    expect(cubicPoint(curve, 10n, 10n)).toEqual([90n, 20n]);
  });
});

describe("mouth (mouth.rs tests)", () => {
  function palette(): Palette {
    const color = (value: bigint): Rgb => ({ r: value, g: value, b: value });
    return {
      back: color(1n),
      primary: color(2n),
      primary_lighter: color(3n),
      primary_darkish: color(4n),
      primary_darker: color(5n),
      primary_darkest: color(6n),
      harmonic1: color(7n),
      harmonic1_darkish: color(8n),
      harmonic1_darkest: color(9n),
      harmonic2: color(10n),
      harmonic3: color(11n),
      harmonic4: color(12n),
      secondary_darker: color(13n),
      dark: color(14n),
      white: color(15n),
    };
  }

  test("every family emits one group and one bounded path", () => {
    const rect = new MouthRect(300_000n, 500_000n, 700_000n, 650_000n);
    for (let family = 0n; family < 8n; family += 1n) {
      const mouth: Mouth = {
        family,
        width_scale_gate: false,
        width_scale: 0n,
        aspect_widen_gate: false,
        params: Array(8).fill(Q32_ONE / 2n),
      };
      const geometry = shapeFor(mouth, rect, MOOD_DELTAS_ZERO, palette());
      for (const [x, y] of geometry.points) {
        expect(x >= rect.x1 && x <= rect.x2 && y >= rect.y1 && y <= rect.y2).toBe(true);
      }
      const svg = renderMouth(mouth, rect, MOOD_DELTAS_ZERO, palette());
      expect(svg.split("<g ").length - 1).toBe(1);
      expect(svg.split("<path ").length - 1).toBe(1);
    }
  });
});

describe("face (face.rs tests)", () => {
  function geometry(): BodyGeometry {
    return {
      path: "",
      bodyWidth: 600_000n,
      bodyHeight: 600_000n,
      top: 100_000n,
      bottom: 700_000n,
      maxWidthY: 420_000n,
      minWidthY: 240_000n,
      allPoints: [],
    };
  }

  test("position modes select the five pinned landmarks", () => {
    const body = geometry();
    const expected = [240_000n, 420_000n, 300_000n, 400_000n, 420_000n];
    expected.forEach((expectedY, mode) => {
      expect(eyeLineAnchor(BigInt(mode), body)).toBe(expectedY);
    });
  });

  test("empty row accepts the reference five percent inset", () => {
    const state = { counter: 0 };
    const rect = findRect([], 600_000n, 300_000n, 50_000n, state);
    expect(rect.x1).toBe(230_000n);
    expect(rect.y1).toBe(300_000n);
    expect(rect.x2).toBe(770_000n);
    expect(rect.y2).toBe(350_000n);
    expect(state.counter).toBe(0);
  });

  test("point sliver terminates with some rectangle", () => {
    const points: Array<[bigint, bigint]> = [
      [500_000n, 500_000n],
      [500_000n, 500_000n],
    ];
    const state = { counter: 0 };
    const rect = findRect(points, 2n, 500_000n, 50_000n, state);
    expect(state.counter <= 3_000 + points.length).toBe(true);
    expect(rect.y2 > rect.y1).toBe(true);
  });
});

describe("svg (svg.rs tests)", () => {
  test("stroke width is the pinned division", () => {
    // 1_000_000 / 255 = 3921.568… → 3922 half away from zero.
    expect(strokeWidthFor()).toBe(3922n);
  });

  test("light z is the pinned division", () => {
    // 1_000_000 / 21 = 47619.04… → 47619.
    expect(lightZ()).toBe(47_619n);
  });

  test("hex colors are lowercase", () => {
    expect(hexColor({ r: 0n, g: 171n, b: 255n })).toBe("#00abff");
    expect(hexColor({ r: 255n, g: 255n, b: 255n })).toBe("#ffffff");
  });

  test("placement is identity for fitting bodies", () => {
    const geometry: BodyGeometry = {
      path: "",
      bodyWidth: 500_000n,
      bodyHeight: 600_000n,
      top: 200_000n,
      bottom: 800_000n,
      maxWidthY: 0n,
      minWidthY: 0n,
      allPoints: [],
    };
    const placement = placementFor(geometry);
    expect(placement.scale).toBe(Q32_ONE);
    expect(placement.tx).toBe(0n);
    // ty = (1_000_000 − 600_000)/2 − 200_000 = 0.
    expect(placement.ty).toBe(0n);
  });

  test("placement scales overflowing bodies down", () => {
    const geometry: BodyGeometry = {
      path: "",
      bodyWidth: 1_200_000n,
      bodyHeight: 600_000n,
      top: 200_000n,
      bottom: 800_000n,
      maxWidthY: 0n,
      minWidthY: 0n,
      allPoints: [],
    };
    const placement = placementFor(geometry);
    expect(placement.scale < Q32_ONE).toBe(true);
    // Width dominates: the scaled width lands on the limit within one
    // milliunit of rounding, and tx centers whatever remains.
    const limit = CANVAS_MILLI - 8n * 3922n;
    const scaledWidth = q32Mul(1_200_000n, placement.scale);
    const widthError = scaledWidth - limit;
    expect(widthError >= -1n && widthError <= 1n).toBe(true);
    const scaledHeight = q32Mul(600_000n, placement.scale);
    expect(placement.tx).toBe(divRoundHalfAway(1_200_000n - scaledWidth, 2n));
    expect(placement.ty).toBe(divRoundHalfAway(CANVAS_MILLI - scaledHeight, 2n) - 200_000n);
  });
});

describe("renderer (lib.rs tests)", () => {
  function defaultParams(): RenderParams {
    return { mood: "neutral", growth: Q32_ONE, canvas: CANVAS_UNITS };
  }

  const point = (x: bigint, y: bigint, tension: bigint): BodyPoint => ({ x, y, tension });

  function bodyWith(points: BodyPoint[]): Body {
    const padding: BodyPoint = { x: 0n, y: 0n, tension: Q32_ONE };
    return {
      point_count: BigInt(points.length),
      size: 800_000n,
      points: [...points, ...Array(16 - points.length).fill(padding)],
    };
  }

  function sampleBody(): Body {
    return bodyWith([
      point(120_000n, 240_000n, Q32_ONE),
      point(360_000n, 480_000n, Q32_ONE),
      point(560_000n, 160_000n, Q32_ONE),
    ]);
  }

  function palette(seed: bigint): Palette {
    const color = (offset: bigint): Rgb => ({
      r: seed & 0xffn,
      g: (seed + offset) & 0xffn,
      b: (seed + offset * 2n) & 0xffn,
    });
    return {
      back: color(1n),
      primary: color(2n),
      primary_lighter: color(3n),
      primary_darkish: color(4n),
      primary_darker: color(5n),
      primary_darkest: color(6n),
      harmonic1: color(7n),
      harmonic1_darkish: color(8n),
      harmonic1_darkest: color(9n),
      harmonic2: color(10n),
      harmonic3: color(11n),
      harmonic4: color(12n),
      secondary_darker: color(13n),
      dark: color(14n),
      white: color(15n),
    };
  }

  /// Builds a complete phenotype around a body and pattern; every other
  /// region is deterministic filler, so render differences trace back to
  /// the fields under test.
  function phenotypeWith(body: Body, paletteSeed: bigint, patternClass: bigint): Phenotype {
    return {
      genome_version: 1n,
      body,
      palette: palette(paletteSeed),
      face: {
        eyes: {
          position_mode: 0n,
          size_h: 0n,
          ipd: 0n,
          height_upper: 0n,
          height_lower: 0n,
          height_bias_gate: false,
          height_bias: 0n,
          vertical_pull_gate: false,
          vertical_pull: 0n,
          contour_inset1: 0n,
          contour_inset2: 0n,
          eyeball_radius_scale: 0n,
          pupil_scale: 0n,
          iris_shade_opacity: 0n,
          iris_shade_reduce: 0n,
          iris_saturation_reduce: 0n,
          lid_close_upper_neutral: 0n,
          lid_close_lower_neutral: 0n,
          lid_extension_gate: 0n,
          lid_extension_side: 0n,
          noise_freq_h: 0n,
          noise_freq_v: 0n,
          noise_seed: 0n,
          rotation_seed: 0n,
          lid_color_gene: 0n,
          iris_color_gene: 0n,
        },
        brows: {
          end_height1: 0n,
          mid_x: 0n,
          end_height2: 0n,
          tensions: [0n, 0n, 0n],
          width_coupling: 0n,
          clearance: 0n,
        },
        lashes: {
          count: 0n,
          removed: 0n,
          length_scale: 0n,
          stroke_scale: 0n,
          direction: 0n,
        },
        mouth: {
          family: 0n,
          width_scale_gate: false,
          width_scale: 0n,
          aspect_widen_gate: false,
          params: [0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n],
        },
        placement: { width_coeff: 0n, height_coeff: 0n },
      },
      pattern: {
        class: patternClass,
        circler: {
          point_reduce: 128n,
          size: Q32_ONE / 2n,
          y_offset_div: Q32_ONE / 2n,
          tension: Q32_ONE / 4n,
          pull_min: Q32_ONE / 2n,
        },
        voronoi: { index: 2n, rotation: 8_192n, scale: Q32_ONE / 2n },
        cut: { spread: Q32_ONE / 4n, y_adjust_div: Q32_ONE / 2n },
      },
      animation: {
        breath_period: 0n,
        breath_amplitude: 0n,
        blink_interval: 0n,
        blink_jitter_seed: 0n,
        mood_transition_tempo: 0n,
      },
      mood_response: {
        lid_reactivity: 0n,
        brow_reactivity: 0n,
        eye_rotation_reactivity: 0n,
        mouth_reactivity: 0n,
      },
      asymmetry: { deltas: [0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n] },
      layers: { count: 0n, params: Array(16).fill(0n) },
    };
  }

  test("every mood is byte identical across repeats", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    for (const mood of ALL_MOODS) {
      const first = renderSvg(phenotype, { ...defaultParams(), mood });
      const second = renderSvg(phenotype, { ...defaultParams(), mood });
      expect(first).toEqual(second);
    }
  });

  test("different phenotypes render differently", () => {
    const params = defaultParams();
    const bodyA = phenotypeWith(sampleBody(), 42n, 0n);
    const bodyB = phenotypeWith(
      bodyWith([
        point(80_000n, 500_000n, Q32_ONE),
        point(400_000n, 200_000n, Q32_ONE),
        point(640_000n, 420_000n, Q32_ONE),
      ]),
      42n,
      0n,
    );
    const paletteB = phenotypeWith(sampleBody(), 200n, 0n);
    const renderA = renderSvg(bodyA, params);
    expect(renderA).not.toEqual(renderSvg(bodyB, params));
    expect(renderA).not.toEqual(renderSvg(paletteB, params));
  });

  test("growth other than one is refused", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    for (const growth of [Q32_ONE - 1n, 0n]) {
      try {
        renderSvg(phenotype, { ...defaultParams(), growth });
        throw new Error("expected RenderError");
      } catch (error) {
        expect(error).toBeInstanceOf(RenderError);
        expect((error as RenderError).kind).toBe("unsupported-growth");
        expect((error as RenderError).value).toBe(growth);
      }
    }
  });

  test("canvas other than pinned is refused", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    for (const canvas of [0n, 9_999n, 10_001n, 255_000n]) {
      try {
        renderSvg(phenotype, { ...defaultParams(), canvas });
        throw new Error("expected RenderError");
      } catch (error) {
        expect(error).toBeInstanceOf(RenderError);
        expect((error as RenderError).kind).toBe("unsupported-canvas");
        expect((error as RenderError).value).toBe(canvas);
      }
    }
  });

  test("body only render remains mood independent", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    const neutral = renderBodySvg(phenotype, defaultParams());
    for (const mood of ALL_MOODS) {
      expect(renderBodySvg(phenotype, { ...defaultParams(), mood })).toEqual(neutral);
    }
  });

  test("neutral is an identity but happy changes the face", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    phenotype.mood_response = {
      lid_reactivity: Q32_ONE,
      brow_reactivity: Q32_ONE,
      eye_rotation_reactivity: Q32_ONE,
      mouth_reactivity: Q32_ONE,
    };
    const neutral = renderSvg(phenotype, defaultParams());
    expect(neutral).toEqual(renderSvg(phenotype, defaultParams()));

    const zeroResponse = structuredClone(phenotype);
    zeroResponse.mood_response = {
      lid_reactivity: 0n,
      brow_reactivity: 0n,
      eye_rotation_reactivity: 0n,
      mouth_reactivity: 0n,
    };
    expect(neutral).toEqual(renderSvg(zeroResponse, defaultParams()));

    expect(neutral).not.toEqual(renderSvg(phenotype, { ...defaultParams(), mood: "happy" }));
  });

  test("output is well formed xml with the pinned structure", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 0n);
    const svg = new TextDecoder().decode(renderSvg(phenotype, defaultParams()));
    assertWellFormedXml(svg);
    expect(svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000">')).toBe(true);
    expect(svg.includes('<path id="av-bp" d="M')).toBe(true);
    expect(svg.includes('<g id="av-face">')).toBe(true);
    const mouth = svg.indexOf('id="av-mouth"');
    const leftEye = svg.indexOf('id="av-eye-left"');
    const rightEye = svg.indexOf('id="av-eye-right"');
    const brows = svg.indexOf('id="av-brow-left"');
    expect(mouth < leftEye && leftEye < rightEye && rightEye < brows).toBe(true);
    // Gradient stops appear in offset order.
    const first = svg.indexOf('offset="89%"');
    const second = svg.indexOf('offset="100%"');
    expect(first < second).toBe(true);
    expect(svg.includes('<rect width="10000" height="10000"')).toBe(true);
  });

  test("patterned output adds mask and pattern use", () => {
    const phenotype = phenotypeWith(sampleBody(), 42n, 4n);
    const svg = new TextDecoder().decode(renderBodySvg(phenotype, defaultParams()));
    assertWellFormedXml(svg);
    expect(svg.split("<path").length - 1).toBe(2);
    expect(svg.includes('<mask id="av-pattern">')).toBe(true);
    expect(svg.includes('mask="url(#av-pattern)"')).toBe(true);
  });

  test("full render is well formed at every mood", () => {
    ALL_MOODS.forEach((mood, index) => {
      const phenotype = phenotypeWith(sampleBody(), 42n, BigInt(index % 6));
      const svg = new TextDecoder().decode(renderSvg(phenotype, { ...defaultParams(), mood }));
      assertWellFormedXml(svg);
    });
  });

  test("two point sliver face search is total", () => {
    const phenotype = phenotypeWith(
      bodyWith([point(2n, 100_000n, Q32_ONE), point(2n, 700_000n, Q32_ONE)]),
      42n,
      0n,
    );
    const svg = new TextDecoder().decode(renderSvg(phenotype, defaultParams()));
    assertWellFormedXml(svg);
    expect(svg.includes('id="av-mouth"')).toBe(true);
  });

  test("bounds are sane across point counts", () => {
    // point_count 2 (degenerate) and 16 (max) plus a mid-range shape.
    const sixteen: BodyPoint[] = [];
    for (let index = 0; index < 16; index += 1) {
      sixteen.push(
        point(
          60_000n + 50_000n * BigInt(index),
          100_000n + 40_000n * BigInt((index * 7) % 11),
          Q32_ONE,
        ),
      );
    }
    const bodies = [
      bodyWith([point(200_000n, 100_000n, Q32_ONE), point(600_000n, 500_000n, Q32_ONE)]),
      bodyWith(sixteen),
      sampleBody(),
    ];
    for (const body of bodies) {
      const geometry = bodyGeometry(phenotypeWith(body, 42n, 0n));
      expect(geometry.allPoints.length > 0).toBe(true);
      expect(geometry.top < geometry.bottom).toBe(true);
      expect(geometry.bodyWidth > 0n).toBe(true);
      expect(geometry.bodyHeight > 0n).toBe(true);
      expect(geometry.bodyHeight).toBe(geometry.bottom - geometry.top);
    }
  });
});

/// Minimal tag-balance checker: enough to prove the output is well-formed
/// XML without an XML dependency. Handles self-closing tags, closing tags,
/// comments, and quoted attribute values. Ported from lib.rs's
/// assert_well_formed_xml.
function assertWellFormedXml(xml: string): void {
  const stack: string[] = [];
  let index = 0;
  while (index < xml.length) {
    if (xml[index] !== "<") {
      index += 1;
      continue;
    }
    if (xml.startsWith("<!--", index)) {
      const end = xml.slice(index).indexOf("-->");
      expect(end).toBeGreaterThanOrEqual(0);
      index += end + 3;
      continue;
    }
    const tagEnd = xml.slice(index).indexOf(">");
    expect(tagEnd).toBeGreaterThanOrEqual(0);
    const tag = xml.slice(index + 1, index + tagEnd);
    if (tag.startsWith("/")) {
      const expected = stack.pop();
      expect(expected).toBeDefined();
      expect(tag.slice(1).trim()).toBe(expected ?? "");
    } else {
      let nameLength = 0;
      while (
        nameLength < tag.length &&
        (/[A-Za-z0-9]/.test(tag[nameLength]) || tag[nameLength] === ":" || tag[nameLength] === "-")
      ) {
        nameLength += 1;
      }
      const name = tag.slice(0, nameLength);
      expect(name.length > 0).toBe(true);
      const attributes = tag.slice(nameLength);
      const quotes = [...attributes].filter((character) => character === '"').length;
      expect(quotes % 2).toBe(0);
      if (!attributes.trimEnd().endsWith("/")) {
        stack.push(name);
      }
    }
    index += tagEnd + 1;
  }
  expect(stack).toEqual([]);
}
