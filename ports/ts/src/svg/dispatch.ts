import { decode, Genome } from "../genome";

import { CANVAS_UNITS, RenderError, renderSvg, type Mood } from "./svg";
import { Q32_ONE } from "./fixed";

/** The stable registry type for the classic creature representation. */
export const CLASSIC = "creature.classic";

const CLASSIC_VERSION = 2;
const CLASSIC_GENOME_BYTES = 1024;

type DispatchErrorKind =
  | "unsupported-type"
  | "unsupported-version"
  | "genome-length"
  | "render";

/** Why a stored avatar could not be rendered. */
export class DispatchError extends Error {
  public readonly requested?: string | number;
  public readonly avatarType?: string;
  public readonly expected?: number;
  public readonly actual?: number;
  public readonly renderError?: RenderError;

  private constructor(
    message: string,
    public readonly kind: DispatchErrorKind,
    fields: {
      requested?: string | number;
      avatarType?: string;
      expected?: number;
      actual?: number;
      renderError?: RenderError;
    } = {},
  ) {
    super(message);
    this.name = "DispatchError";
    this.requested = fields.requested;
    this.avatarType = fields.avatarType;
    this.expected = fields.expected;
    this.actual = fields.actual;
    this.renderError = fields.renderError;
  }

  static unsupportedType(requested: string): DispatchError {
    return new DispatchError(`unsupported avatar type ${JSON.stringify(requested)}`, "unsupported-type", {
      requested,
    });
  }

  static unsupportedVersion(avatarType: string, requested: number): DispatchError {
    return new DispatchError(
      `unsupported avatar version ${requested} for type ${JSON.stringify(avatarType)}`,
      "unsupported-version",
      { avatarType, requested },
    );
  }

  static genomeLength(expected: number, actual: number): DispatchError {
    return new DispatchError(
      `genome must contain ${expected} bytes, received ${actual}`,
      "genome-length",
      { expected, actual },
    );
  }

  static render(renderError: RenderError): DispatchError {
    return new DispatchError(renderError.message, "render", { renderError });
  }
}

/**
 * Renders the type and generation version named by a registry row.
 *
 * The route never selects a newer generation: when a version is added, retain
 * the classic v2 arm below and add another explicit arm for the new version.
 */
export function renderSvgFor(
  genome: Genome,
  type: string,
  version: number,
  mood: Mood,
): Uint8Array {
  return renderSvgForBytes(genome.asBytes(), type, version, mood);
}

function renderSvgForBytes(
  genomeBytes: Uint8Array,
  type: string,
  version: number,
  mood: Mood,
): Uint8Array {
  if (type !== CLASSIC) throw DispatchError.unsupportedType(type);
  if (version !== CLASSIC_VERSION) throw DispatchError.unsupportedVersion(type, version);
  if (genomeBytes.length !== CLASSIC_GENOME_BYTES) {
    throw DispatchError.genomeLength(CLASSIC_GENOME_BYTES, genomeBytes.length);
  }

  try {
    return renderSvg(decode(Genome.fromBytes(genomeBytes)), {
      mood,
      growth: Q32_ONE,
      canvas: CANVAS_UNITS,
    });
  } catch (error) {
    if (error instanceof RenderError) throw DispatchError.render(error);
    throw error;
  }
}
